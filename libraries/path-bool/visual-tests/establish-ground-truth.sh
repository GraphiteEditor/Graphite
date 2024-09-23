for dir in */; do
	for fn in difference division exclusion fracture intersection union; do
		cp "${dir}test-results/$fn-ours.svg" "$dir$fn.svg"
	done
done
