#!/bin/bash
function count_diagnostics() {
	if [ ! -f Cargo.toml ]; then
		return
	fi
	rust-diagnostics
	find diagnostics -name "*.rs" | while read f; do
		tree-grepper -q rust "(block_comment)" $f | grep "^\#\[Warning" 
	done > w.txt 
	wc w.txt
	cat w.txt | sort | uniq -c | sort -n
	d=$(cat w.txt | sort | uniq -c | sort -n | awk '{s=s+$1}END{print s}')
	echo "Number of warnings = $d"
        a=$(tokei -t=Rust src | grep " Total" | awk '{print $3}')
        echo "Lines of Rust code: $a"
	f=$(( d * 1000 / a ))
	echo "Number of warnings per KLOC: $d * 1000 / $a = $f"
}
export -f count_diagnostics
count_diagnostics
