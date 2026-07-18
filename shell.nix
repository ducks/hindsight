{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    ruby_3_2
    git
  ];

  shellHook = ''
    # Prevent loading incompatible gems from ~/.local/share/gem
    export GEM_HOME=""
    export GEM_PATH=""

    echo "hindsight dev environment"
    echo "Ruby $(ruby --version)"
    echo ""
    echo "Available commands:"
    echo "  bin/ingest-incidents    - Ingest incidents (run via Claude Code)"
    echo "  bin/migrate-v1-to-v2    - Migrate v1 data to v2 with date-ver"
  '';
}
