#!/usr/bin/env bash
set -euo pipefail
if [[ $# -lt 1 ]]; then
  printf 'Uso: bash iniciar-sincronizacion.sh /ruta/absoluta/servidor [--excluir-archivo mod.jar ...] [--incluir-archivo mod.jar ...] [--incluir-servidor]\n' >&2
  exit 1
fi
servidor=$1
shift
raiz=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
binario=${ICARO_SYNC_BIN:-"$raiz/target/release/examples/sync_host"}
exec "$binario" --servidor "$servidor" --minecraft 1.21.1 --loader fabric \
  --version-loader 0.19.5 --escuchar 10.62.50.154:25566 "$@"
