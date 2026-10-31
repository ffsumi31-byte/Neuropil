{
  description = "3D AI Knowledge Brain - Polyglot Dev Environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
      pythonEnv = pkgs.python3.withPackages (ps: with ps; [
        requests
        wikipedia
         sentence-transformers
      ]);
    in {
      devShells.${system}.default = pkgs.mkShell {
        buildInputs = with pkgs; [
          rustc
          cargo
          pythonEnv
          nodejs_22
          nodePackages.typescript
        ];
      };
    };
}
