set -eu
dtc -I dtb -O dts -o "$2" "$1"
