use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use chrono::naive::NaiveDate as Date;
use clap::Parser;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use anyhow::Result;

const ERROR: f64 = 0.001;
const NOTPRESENT: f64 = f64::NAN;
const NO_MAPSTATS: MapStats = MapStats {
  kills: NOTPRESENT,
  assists: NOTPRESENT,
  deaths: NOTPRESENT,
  hs: NOTPRESENT,
  kast: NOTPRESENT, 
  first_kill_diff: NOTPRESENT,
  adr: NOTPRESENT,
  rating: NOTPRESENT,
};

#[derive(Debug, serde::Deserialize)]
struct MatchStats {
  date: Date,
  team_1: String, team_2: String,
  _map: String,
  result_1: u8,
  result_2: u8,
  map_winner: u8,
  starting_ct: u8,
  ct_1: u8,
  t_2: u8,
  t_1: u8,
  ct_2: u8,
  event_id: u32,
  match_id: u32,
  rank_1: u32,
  rank_2: u32,
  map_wins_1: u32,
  map_wins_2: u32,
  match_winner: u32
}

#[derive(Clone, Debug)]
struct PlayerMatch {
  date: Date,
  match_id: u32,
  team: String,
  player_id: u32,
  match_: Match
}

impl PlayerMatch {
  fn as_stats(&self) -> RelevantStats {
    match &self.match_ {
      Match::Bo1(m) => RelevantStats { map_stats: m.clone(), weight: 1.0 },
      Match::Bo3 { first, second, third } => {
        let mut weight = 0.0;
        let mut map_stats = MapStats::default();
        if let Some(m) = first {
          map_stats.merge(&m);
          weight += 1.0;
        }
        if let Some(m) = second {
          map_stats.merge(&m);
          weight += 1.0;
        }
        if let Some(m) = third {
          map_stats.merge(&m);
          weight += 1.0;
        }
        RelevantStats { map_stats, weight }
      }
    }
  }
}

#[derive(Debug, Clone, Default, serde::Serialize)]
struct MapStats {
  kills: f64,
  assists: f64,
  deaths: f64,
  hs: f64,
  first_kill_diff: f64,
  kast: f64,
  adr: f64,
  rating: f64,
}

impl MapStats {
  fn scale(&mut self, scale: f64) {
    self.kills *= scale;
    self.assists *= scale;
    self.deaths *= scale;
    self.hs *= scale;
    self.first_kill_diff *= scale;
    self.kast *= scale;
    self.adr *= scale;
    self.rating *= scale;
  }
}

#[derive(Clone, Debug)]
enum Match {
  Bo1(MapStats),
  Bo3 {
    first: Option<MapStats>,
    second: Option<MapStats>,
    third: Option<MapStats>
  }
}

#[derive(Debug, serde::Serialize, Clone)]
struct RelevantStats {
  map_stats: MapStats,
  weight: f64,
}

#[derive(serde::Serialize)]
struct RowData {
  players: [MapStats; 10],
  winner: u8
}

impl MapStats {
  fn merge(&mut self, other: &Self) {
    self.kills += other.kills;
    self.assists += other.assists;
    self.deaths += other.deaths;
    self.hs += other.hs;
    self.first_kill_diff += other.first_kill_diff;
    self.kast += other.kast;
    self.adr += other.adr;
    self.rating += other.rating;
  }
}

fn new_record(maps: &mut HashMap<u32, BTreeMap<Date, Option<RelevantStats>>>, player_match: PlayerMatch) {
  let periods = maps.entry(player_match.player_id);
  let mut stats = player_match.as_stats();
  if stats.weight == 0.0 {
    return
  }
  match periods {
    Occupied(mut occ) => {
      for (&period_end, maybe_stats) in occ.get_mut().iter_mut() {
        let time_diff = (period_end - player_match.date).num_days() as f64;
        let scale = (-time_diff.sqrt()).exp();
        stats.map_stats.scale(scale);
        stats.weight *= scale;
        if let Some(RelevantStats { map_stats: past_stats, weight }) = maybe_stats {
          past_stats.merge(&stats.map_stats);
          *weight += stats.weight;
        }
        else {
          *maybe_stats = Some(stats.clone());
        }
      }
      occ.get_mut().entry(player_match.date).or_insert(None);
    },
    Vacant(vac) => {
      let periods = BTreeMap::from([(player_match.date, None)]);
      vac.insert(periods);
    },
  }
}

fn parse_matches() -> Result<HashMap<u32, MatchStats>> {
  let file = std::fs::File::open("data/results.csv")?;
  let buf_read = std::io::BufReader::new(file);
  let mut reader = csv::Reader::from_reader(buf_read);
  reader
    .deserialize()
    .map(|record| {
      let played_match: MatchStats = record?;
      Ok((played_match.match_id, played_match))
    })
    .collect::<Result<HashMap<u32, MatchStats>>>()
}

fn parse_match(row: &csv::StringRecord) -> Result<PlayerMatch> {
  fn parse_stats(row: &csv::StringRecord, map_idx: usize) -> Result<MapStats> {
    let get_float = |i: usize | -> Result<f64> {
      Ok(row.get( i + 10 * map_idx)
        .ok_or_else(|| anyhow::format_err!("could not get index {i}"))?
        .parse::<f64>()?)
    };
    let kills = get_float(23)?;
    let assists = get_float(24)?;
    let deaths = get_float(25)?;
    let hs = get_float(26)?;
    let kast = get_float(28)?;
    let adr = get_float(30)?;
    let first_kill_diff = get_float(31)?;
    let rating = get_float(32)?;
    Ok(MapStats {
      kills,
      assists,
      deaths,
      hs,
      kast,
      adr,
      first_kill_diff,
      rating
    })
  }
  let get = |i: usize| row.get(i).ok_or_else(|| anyhow::format_err!("could not get index {i}"));
  let date = Date::parse_from_str(get(0)?, "%Y-%m-%d").expect("valid date");
  let team = get(2).expect("invalid string").to_string();
  let player_id = get(5)?.parse::<u32>()?;
  let match_id = get(6)?.parse::<u32>()?;
  let best_of = get(9)?.parse::<u8>()?;
  let match_ = if best_of == 1 {
    Match::Bo1(parse_stats(row, 0)?)
  } else {
    let first = parse_stats(row, 0).ok();
    let second = parse_stats(row, 1).ok();
    let third = parse_stats(row, 2).ok();
    Match::Bo3 { first, second, third }
  };
  Ok(PlayerMatch {
    date,
    match_id,
    team,
    player_id,
    match_
  })
}

fn parse_historic_data() -> Result<(HashMap<u32, BTreeMap<Date, Option<RelevantStats>>>, HashMap<u32, Vec<PlayerMatch>>)> {
  let file = std::fs::File::open("data/players.csv")?;
  let buf_read = std::io::BufReader::new(file);
  let mut reader = csv::Reader::from_reader(buf_read);
  let mut historical_data: HashMap<u32, BTreeMap<Date, Option<RelevantStats>>> = HashMap::with_capacity(400_000);
  let mut players_in_match: HashMap<u32, Vec<PlayerMatch>> = HashMap::with_capacity(50_000);
  for result in reader.records() {
    let record = result?;
    if let Ok(player_match) = parse_match(&record) {
      new_record(&mut historical_data, player_match.clone());
      players_in_match.entry(player_match.match_id.clone()).and_modify(|vec| vec.push(player_match.clone())).or_insert(vec![player_match]);
    }
  }
  Ok((historical_data, players_in_match))
}

fn to_row_data(
  matches: HashMap<u32, MatchStats>,
  players: HashMap<u32, BTreeMap<Date, Option<RelevantStats>>>,
  players_matches: HashMap<u32, Vec<PlayerMatch>>,
) -> Vec<RowData> {
  let mut row_data: Vec<RowData> = Vec::with_capacity(50_000);
  for (match_id, match_stats) in matches {
    let Some(all_players_in_match) = players_matches.get(&match_id) else { continue };
    let (team_1_players , team_2_players): (Vec<_>, Vec<_>) = all_players_in_match.iter().partition(|p| p.team == match_stats.team_1);
    if team_1_players.len() + team_2_players.len() > 10 {
      continue // not sure what to do in this case
    }
    let team_data = |team_players: Vec<&PlayerMatch>| -> [MapStats; 5] {
      let get_hist_data = |player_idx| -> Option<MapStats> {
        let player_match: &&PlayerMatch = team_players.get(player_idx)?;
        let stats_map = players.get(&player_match.player_id)?;
        let mut s = stats_map.get(&player_match.date)?.clone()?;
        if -ERROR <= s.weight && s.weight <= ERROR  {
          return None;
        }
        s.map_stats.scale(1.0 / s.weight);
        Some(s.map_stats)
      };
      [
        get_hist_data(0).unwrap_or(NO_MAPSTATS),
        get_hist_data(1).unwrap_or(NO_MAPSTATS),
        get_hist_data(2).unwrap_or(NO_MAPSTATS),
        get_hist_data(3).unwrap_or(NO_MAPSTATS),
        get_hist_data(4).unwrap_or(NO_MAPSTATS)
      ]
    };
    let [t1p1, t1p2, t1p3, t1p4, t1p5] = team_data(team_1_players);
    let [t2p1, t2p2, t2p3, t2p4, t2p5] = team_data(team_2_players);
    let row = RowData {
      players: [
        t1p1, t1p2, t1p3, t1p4, t1p5,
        t2p1, t2p2, t2p3, t2p4, t2p5
      ],
      winner: match_stats.map_winner - 1
    };
    row_data.push(row)
  }
  row_data
}

#[derive(Parser, Debug)]
struct Args {
  filename: String,
  #[arg(long)]
  kills: bool,
  #[arg(long)]
  assists: bool,
  #[arg(long)]
  deaths: bool,
  #[arg(long)]
  hs: bool,
  #[arg(long)]
  kast: bool,
  #[arg(long)]
  fkdiff: bool,
  #[arg(long)]
  adr: bool,
  #[arg(long)]
  rating: bool,
}

fn write_row(cfg: &Args, row: &RowData, file: &mut csv::Writer<std::fs::File>) -> Result<()> {
  for p in row.players.iter() {
    if cfg.kills {
      file.write_field(p.kills.to_string())?;
    }
    if cfg.assists {
      file.write_field(p.assists.to_string())?;
    }
    if cfg.deaths {
      file.write_field(p.deaths.to_string())?;
    }
    if cfg.hs {
      file.write_field(p.hs.to_string())?;
    }
    if cfg.kast {
      file.write_field(p.kast.to_string())?;
    }
    if cfg.fkdiff {
      file.write_field(p.first_kill_diff.to_string())?;
    }
    if cfg.adr {
      file.write_field(p.adr.to_string())?;
    }
    if cfg.rating {
      file.write_field(p.rating.to_string())?;
    }
  }
  file.write_field(row.winner.to_string())?;
  file.write_record(None::<&[u8]>)?;
  Ok(())
}

fn main() -> Result<()> {
  let args = Args::parse();
  let matches = parse_matches()?;
  let (players, players_matches) = parse_historic_data()?;
  let mut output = csv::WriterBuilder::new()
    .has_headers(false)
    .from_path(args.filename.clone())?;
  for row in to_row_data(matches, players, players_matches) {
    write_row(&args, &row, &mut output)?;
  }
  Ok(())
}
