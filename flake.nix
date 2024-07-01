{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { nixpkgs, rust-overlay, ... }: let
    system = "x86_64-linux";
    pkgs = import nixpkgs {
      inherit system;
      overlays = [ (import rust-overlay) ];
    };
    python-environment = pkgs.python3.withPackages(p: with p; [
      pandas
      ipython
      jupyter
      matplotlib
      numpy
      tensorflow
      keras
      optree
      rich
      ml-dtypes
      scikit-learn
      graphviz
      seaborn
    ]);
    tex-environment = pkgs.texlive.combine {
      inherit (pkgs.texlive) scheme-medium
        # wrapfig ulem capt-of
        caption
        etoolbox #geometry
        babel-portuges
        minted
        upquote
        titlesec;
    };
  in {
    packages.x86_64-linux.default = pkgs.stdenvNoCC.mkDerivation rec {
      name = "IA-relatorio";
      src = ./relatorio;
      buildInputs = [ pkgs.coreutils tex-environment];
      phases = ["unpackPhase" "buildPhase" "installPhase"];
      buildPhase = ''
        export PATH="${pkgs.lib.makeBinPath buildInputs}";
        mkdir -p .cache/texmf-var
        env TEXMFHOME=.cache TEXMFVAR=.cache/texmf-var \
          latexmk -interaction=nonstopmode -pdf -lualatex \
          ${./relatorio/relatorio.tex}
      '';
      installPhase = ''
        mkdir -p $out
        cp relatorio.pdf $out/
      '';
    };
    devShells.x86_64-linux.default = with pkgs; mkShell {
      buildInputs = [
        python-environment
        (rust-bin.stable."1.77.0".default.override {
          extensions = ["rust-src" "rust-analyzer"];
        })
        tex-environment
      ];
    };
  };
}
