set -eu
bios="$1"
shift
nice -n19 qemu-system-riscv32 -nographic -machine virt -m 128 -smp 1 \
    -bios "$bios" \
    "$@"
