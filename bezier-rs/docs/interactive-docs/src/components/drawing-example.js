const drawLine = (ctx, p1, p2,) => {
	ctx.beginPath();
	ctx.moveTo(p1.x, p1.y);
	ctx.lineTo(p2.x, p2.y);
	ctx.stroke();

	drawPoint(ctx, p1)
	drawPoint(ctx, p2)
}

const drawPoint = (ctx, p) => {
	ctx.beginPath();
	ctx.arc(p.x, p.y, p.r, 0, 2 * Math.PI, false);
	ctx.fillStyle = 'black';
	ctx.fill();

	if (p.selected) {
		ctx.beginPath();
		ctx.arc(p.x, p.y, p.r / 1.5, 0, 2 * Math.PI, false);
		ctx.fillStyle = 'blue';
		ctx.fill();
	}
}


class DrawingExample {
    constructor(component) {
        let canvas = document.createElement("canvas");
        canvas.width = 200;
        canvas.height = 200;
		component.appendChild(canvas);

		let ctx = canvas.getContext("2d");
        let points = [{x:30, y:30, r:5}, {x:150, y: 100, r:5}]
		let dragIndex = null // Index of the point being moved

        canvas.addEventListener("mousedown", function(evt) {
            let mx = evt.offsetX;
            let my = evt.offsetY;

			for (let i = 0; i < points.length; i++) {
                if (Math.abs(mx - points[i].x) < points[i].r && 
					Math.abs(my - points[i].y) < points[i].r) {
					dragIndex = i;
					points[dragIndex].selected = true
					break
                }
            }
        });
        canvas.addEventListener("mousemove", function(evt) {
            let mx = evt.offsetX;
            let my = evt.offsetY;

			if (dragIndex != null) {
				points[dragIndex].x = mx
				points[dragIndex].y = my		
				ctx.clearRect(0, 0, canvas.width, canvas.height)
				drawLine(ctx, points[0], points[1]);
			}
        });

		const deselectPointHandler = () => {
			if (dragIndex != null) {
				points[dragIndex].selected = false
				drawLine(ctx, points[0], points[1]);
				dragIndex = null
			}
		}
        canvas.addEventListener("mouseup", deselectPointHandler);
		canvas.addEventListener("mouseout", deselectPointHandler);

        drawLine(ctx, points[0], points[1]);
    }
}
export { DrawingExample };
