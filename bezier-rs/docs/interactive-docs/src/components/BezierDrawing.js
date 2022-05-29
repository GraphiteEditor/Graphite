const drawLine = (ctx, p1, p2) => {
	ctx.strokeStyle = 'grey';
	ctx.lineWidth = 1;

	ctx.beginPath();
	ctx.moveTo(p1.x, p1.y);
	ctx.lineTo(p2.x, p2.y);
	ctx.stroke();
}

const drawPoint = (ctx, p) => {
	// Outline the point
	ctx.strokeStyle = p.selected? 'blue' : 'black';
	ctx.lineWidth = p.r / 3;
	ctx.beginPath();
	ctx.arc(p.x, p.y, p.r, 0, 2 * Math.PI, false);
	ctx.stroke();

	// Fill the point (hiding any overlapping lines)
	ctx.fillStyle = 'white'
	ctx.beginPath();
	ctx.arc(p.x, p.y, p.r * (2/3), 0, 2 * Math.PI, false);
	ctx.fill();

}

const drawBezier = (ctx, points) => {
	/* Until a bezier representation is finalized, treat the points as follows
		points[0] = left endpoint
		points[1] = right endpoint
		points[2] = handle 1
		points[3] = (optional) handle 2
	*/
	let left = points[0]
	let right = points[1]
	let handle1 = points[2]
	let handle2 = points.length == 4? points[3] : points[2]

	ctx.strokeStyle = "black"
	ctx.lineWidth = 2;

	ctx.beginPath();
	ctx.moveTo(points[0].x, points[0].y);
	if (points.length == 3) {
		ctx.quadraticCurveTo(handle1.x, handle1.y, right.x, right.y);
	} else {
		ctx.bezierCurveTo(handle1.x, handle1.y, handle2.x, handle2.y, right.x, right.y);
	}
	ctx.stroke();

	drawLine(ctx, left, handle1)
	drawLine(ctx, right, handle2)

	for (let point of points) {
		drawPoint(ctx, point)
	}
}


class BezierDrawing {
    constructor(component, points) {
        let canvas = document.createElement("canvas");
        canvas.width = 200;
        canvas.height = 200;
		component.appendChild(canvas);

		let ctx = canvas.getContext("2d");
		let dragIndex = null // Index of the point being moved

        canvas.addEventListener("mousedown", function(evt) {
            let mx = evt.offsetX;
            let my = evt.offsetY;

			for (let i = 0; i < points.length; i++) {
                if (Math.abs(mx - points[i].x) < points[i].r+3 && // +3 makes the points easier to grab
					Math.abs(my - points[i].y) < points[i].r+3) {
					dragIndex = i;
					points[dragIndex].selected = true
					break
                }
            }
        });
        canvas.addEventListener("mousemove", function(evt) {
            let mx = evt.offsetX;
            let my = evt.offsetY;

			if (dragIndex != null && (
				mx - points[dragIndex].r > 0 &&
				my - points[dragIndex].r > 0 && 
				mx + points[dragIndex].r < canvas.width &&
				my + points[dragIndex].r < canvas.height
			)) {
				points[dragIndex].x = mx
				points[dragIndex].y = my		
				ctx.clearRect(1, 1, canvas.width-2, canvas.height-2)
				drawBezier(ctx, points);
			}
        });

		const deselectPointHandler = () => {
			if (dragIndex != null) {
				points[dragIndex].selected = false
				ctx.clearRect(1, 1, canvas.width-2, canvas.height-2)
				drawBezier(ctx, points);
				dragIndex = null
			}
		}
        canvas.addEventListener("mouseup", deselectPointHandler);
		canvas.addEventListener("mouseout", deselectPointHandler);

		ctx.strokeRect(0, 0, canvas.width, canvas.height)
        drawBezier(ctx, points);
    }
}
export { BezierDrawing };
