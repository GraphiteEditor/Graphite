INKSCAPE_CMD=inkscape
OPS=(union difference intersection exclusion division fracture)

for dir in */; do
	for op in "${OPS[@]}"; do
		if [ ! -e "$dir/$op.svg" ]; then
			$INKSCAPE_CMD --actions="select-all; path-$op; export-filename:$dir/$op.svg; export-plain-svg; export-do; file-close" "$dir/original.svg"
		fi
	done
done
