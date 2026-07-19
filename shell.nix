{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    python311
    git
  ];

  shellHook = ''
    echo "hindsight dev environment"
    echo "Python $(python3 --version)"
    echo ""
    echo "Available commands:"
    echo "  test_discourse_ingester.py  - Test Python ingester"
  '';
}
