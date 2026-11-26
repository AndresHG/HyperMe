#!/bin/zsh
# usage: gpu.sh outfile  — samples GPU utilisation every 0.5 s until killed
mkdir -p "$(dirname "$1")"
while true; do echo "$(date +%s.%N) $(ioreg -r -d 1 -w 0 -c IOAccelerator | grep -o '"Device Utilization %"=[0-9]*' | head -1 | cut -d= -f2)" >> $1; sleep 0.5; done
