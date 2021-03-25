set -eu
src="$1"
./h.py <"$src" >bios && exec ./sim.sh bios
