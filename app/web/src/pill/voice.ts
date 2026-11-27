// The voice in the pill: a row of white bars that ripple out from the centre with the input level
// while recording (U-14), and a line with a small violet light while HyperMe works (U-15).
// Drawn on a canvas, 30 frames a second, only while the pill is visible and moving.

const BARS = 11;
const BAR_W = 3;
const GAP = 3;
const HEIGHT = 20;
const MIN = 3;
export const WIDTH = BARS * BAR_W + (BARS - 1) * GAP;
const CENTRE = (BARS - 1) / 2;
const FRAME_MS = 1000 / 30; // 30 frames a second, only while visible
const SMOOTH = 1 - Math.exp(-FRAME_MS / 80); // about 80 ms of smoothing
const SETTLE_MS = 160; // bars to line after release (U-15)
const LIGHT_MS = 1400; // one run of the light, there and back
const TRACK = '#3a3a44';
const LIGHT = '160,140,255';
// Taller in the middle, so the row reads as a voice.
const shape = Array.from({ length: BARS }, (_, i) => 0.55 + 0.45 * Math.cos((((i - CENTRE) / (CENTRE + 1)) * Math.PI) / 2));

/** What the voice draws: the bars, the working line, or nothing. */
export type Motion = 'voice' | 'working' | 'still';

export class Voice {
  private readonly ctx: CanvasRenderingContext2D;
  private motion: Motion = 'still';
  private target = 0; // the last level from the microphone, 0 to 1
  private smooth = 0;
  private readonly history = new Array<number>(CENTRE + 1).fill(0); // [0] now, [d] d frames ago
  private readonly heights = new Array<number>(BARS).fill(MIN);
  private since = 0; // when the current motion began
  private raf = 0;
  private lastFrame = 0;
  /** Frames drawn, for the check that nothing runs while hidden. */
  frames = 0;

  constructor(
    canvas: HTMLCanvasElement,
    private readonly reduce: MediaQueryList,
  ) {
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('no 2D canvas');
    this.ctx = ctx;
    const ratio = window.devicePixelRatio || 1;
    canvas.width = Math.round(WIDTH * ratio);
    canvas.height = Math.round(HEIGHT * ratio);
    canvas.style.width = WIDTH + 'px';
    canvas.style.height = HEIGHT + 'px';
    ctx.scale(ratio, ratio);
    document.addEventListener('visibilitychange', () => (document.hidden ? this.stop() : this.animate()));
  }

  /** The microphone's level, 0 to 1. */
  level(value: number) {
    this.target = value;
  }

  /** Switches what is drawn. Entering the bars draws their first, flat frame now, inside the
   *  50 ms key-down budget. */
  set(motion: Motion) {
    const was = this.motion;
    this.motion = motion;
    this.since = performance.now();
    if (motion === 'voice' && was !== 'voice') {
      this.target = this.smooth = 0;
      this.history.fill(0);
      this.heights.fill(MIN);
      this.lastFrame = this.since;
      this.ctx.clearRect(0, 0, WIDTH, HEIGHT);
      this.drawBars(() => MIN);
    }
    if (motion === 'still') this.stop();
    else this.animate();
  }

  private step(now: number) {
    const reduce = this.reduce.matches;
    this.smooth += (this.target - this.smooth) * SMOOTH;
    this.history.pop();
    this.history.unshift(this.smooth);
    for (let i = 0; i < BARS; i++) {
      // Each bar takes the level a frame after its neighbour nearer the centre. With Reduce
      // motion they all take the level now, without the ripple or the idle stir.
      const value = reduce ? this.smooth : (this.history[Math.round(Math.abs(i - CENTRE))] ?? 0);
      // Silence keeps the bars low but never flat, so the pill still looks alive.
      const alive = reduce ? 0 : 0.6 * (1 + Math.sin(now / 260 + i * 0.9));
      this.heights[i] = Math.min(HEIGHT, MIN + alive + (HEIGHT - MIN) * value * (shape[i] ?? 1));
    }
  }

  private drawBars(heightOf: (i: number) => number) {
    const ctx = this.ctx;
    ctx.fillStyle = '#ffffff';
    for (let i = 0; i < BARS; i++) {
      const h = heightOf(i);
      ctx.beginPath();
      ctx.roundRect(i * (BAR_W + GAP), (HEIGHT - h) / 2, BAR_W, h, BAR_W / 2);
      ctx.fill();
    }
  }

  private drawWorking(now: number) {
    const ctx = this.ctx;
    const t = now - this.since;
    if (t < SETTLE_MS && !this.reduce.matches) {
      const k = 1 - Math.pow(1 - t / SETTLE_MS, 3);
      this.drawBars((i) => (this.heights[i] ?? MIN) + (2 - (this.heights[i] ?? MIN)) * k);
      return;
    }
    const y = HEIGHT / 2 - 1;
    ctx.fillStyle = TRACK;
    ctx.beginPath();
    ctx.roundRect(0, y, WIDTH, 2, 1);
    ctx.fill();
    // A small violet light runs along the line until the text lands. With Reduce motion it
    // stays still in the middle.
    const light = 24;
    const phase = this.reduce.matches ? 0.5 : (1 - Math.cos(((t - SETTLE_MS) / LIGHT_MS) * 2 * Math.PI)) / 2;
    const x = phase * (WIDTH - light);
    const glow = ctx.createLinearGradient(x, 0, x + light, 0);
    glow.addColorStop(0, `rgba(${LIGHT},0)`);
    glow.addColorStop(0.5, `rgba(${LIGHT},1)`);
    glow.addColorStop(1, `rgba(${LIGHT},0)`);
    ctx.fillStyle = glow;
    ctx.beginPath();
    ctx.roundRect(x, y - 0.5, light, 3, 1.5);
    ctx.fill();
  }

  private readonly frame = (now: number) => {
    this.raf = 0;
    if (this.motion === 'still') return;
    if (now - this.lastFrame >= FRAME_MS - 2) {
      this.lastFrame = now;
      this.frames++;
      this.ctx.clearRect(0, 0, WIDTH, HEIGHT);
      if (this.motion === 'voice') {
        this.step(now);
        this.drawBars((i) => this.heights[i] ?? MIN);
      } else {
        this.drawWorking(now);
      }
    }
    if (!document.hidden) this.raf = requestAnimationFrame(this.frame);
  };

  private animate() {
    if (!this.raf && this.motion !== 'still') this.raf = requestAnimationFrame(this.frame);
  }

  private stop() {
    if (this.raf) cancelAnimationFrame(this.raf);
    this.raf = 0;
  }
}
