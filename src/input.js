"use strict";

(function initInput(RP) {
  function createPointer(canvas) {
    const pointer = {
      x: 0,
      y: 0,
      px: 0,
      py: 0,
      vx: 0,
      vy: 0,
      down: false,
      active: false,
      power: 2
    };

    function update(event) {
      const rect = canvas.getBoundingClientRect();
      const x = event.clientX - rect.left;
      const y = event.clientY - rect.top;
      pointer.vx = x - pointer.x;
      pointer.vy = y - pointer.y;
      pointer.px = pointer.x;
      pointer.py = pointer.y;
      pointer.x = x;
      pointer.y = y;
      pointer.active = true;
    }

    canvas.addEventListener("pointermove", update);
    canvas.addEventListener("pointerdown", (event) => {
      update(event);
      pointer.down = true;
      canvas.setPointerCapture(event.pointerId);
    });
    canvas.addEventListener("pointerup", (event) => {
      update(event);
      pointer.down = false;
    });
    canvas.addEventListener("pointerleave", () => {
      pointer.active = false;
      pointer.down = false;
    });

    return pointer;
  }

  RP.Input = {
    createPointer
  };
})(window.RP);
