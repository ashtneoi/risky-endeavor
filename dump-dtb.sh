set -eu
output="$1"
shift
qemu-system-riscv64 -machine virt,dumpdtb="$output" "$@"
