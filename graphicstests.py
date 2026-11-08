
import tkinter as tk

WIDTH = 600
HEIGHT = 600

root = tk.Tk()
root.title("Ellipse Scanline Test")

canvas = tk.Canvas(
    root,
    width=WIDTH,
    height=HEIGHT,
    bg="#111111",
    highlightthickness=0
)
canvas.pack()

def draw_ellipse(cx, cy, rx, ry):
    canvas.delete("all")

    x = 0
    y = ry

    rx2 = rx * rx
    ry2 = ry * ry

    # Region 1
    dx = 2 * ry2 * x
    dy = 2 * rx2 * y

    p1 = ry2 - rx2 * ry + 0.25 * rx2

    while dx < dy:
        # Mirror across both axes
        canvas.create_line(
            cx - x, cy - y,
            cx + x, cy - y,
            fill="#00e5c3"
        )
        canvas.create_line(
            cx - x, cy + y,
            cx + x, cy + y,
            fill="#00e5c3"
        )

        x += 1
        dx = 2 * ry2 * x

        if p1 < 0:
            p1 += dx + ry2
        else:
            y -= 1
            dy = 2 * rx2 * y
            p1 += dx - dy + ry2

    # Region 2
    p2 = (
        ry2 * (x + 0.5)**2
        + rx2 * (y - 1)**2
        - rx2 * ry2
    )

    while y >= 0:
        canvas.create_line(
            cx - x, cy - y,
            cx + x, cy - y,
            fill="#00e5c3"
        )
        canvas.create_line(
            cx - x, cy + y,
            cx + x, cy + y,
            fill="#00e5c3"
        )

        y -= 1
        dy = 2 * rx2 * y

        if p2 > 0:
            p2 += rx2 - dy
        else:
            x += 1
            dx = 2 * ry2 * x
            p2 += dx - dy + rx2

    canvas.create_line(cx, 0, cx, HEIGHT, fill="#444444")
    canvas.create_line(0, cy, WIDTH, cy, fill="#444444")

draw_ellipse(WIDTH / 2, HEIGHT / 2, 100, 100)

root.mainloop()
