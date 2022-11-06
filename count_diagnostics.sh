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
        a=$(tokei -t=Rust | grep " Total" | awk '{print $3}')
        b=$(tokei -t=Rust diagnostics | grep " Total" | awk '{print $3}')
	c=$(( a - b ))
        echo "Lines of Rust code: $a - $b  = $c"
	f=$(( d * 1000 / c ))
	echo "Number of warnings per KLOC: $d * 1000 / $c = $f"
}
export -f count_diagnostics
count_diagnostics
