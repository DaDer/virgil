// The clock (ux-spec 1.4): one moment shared by every view. Live: now, advancing once per
// second. Replay: the playhead, interpolated between status updates while playing.
// Controls forward to the data source's replay commands.

import { errorText } from '../data/source';
import { world } from './world.svelte';

export const SPEEDS = [1, 2, 5, 10, 30, 60, 120] as const;
const SEEK_THROTTLE_MS = 100;

class ClockStore {
  /** Wall clock; ticks once per second, every frame while a replay plays. */
  wall = $state(Date.now());
  /** Position being dragged on a scrubber; shown at once, sent to the core at most 10x/s. */
  scrubbing = $state<number | null>(null);
  /** The toggle before the core has confirmed it; the replay state wins once there is one. */
  private skipGapsWanted = $state(true);
  lastError = $state<string | null>(null);
  private seekTimer: ReturnType<typeof setTimeout> | null = null;
  private lastSeekWall = 0;
  private started = false;

  get mode(): 'live' | 'replay' {
    return world.mode;
  }

  get replay() {
    return world.status?.replay ?? null;
  }

  get playing(): boolean {
    return !!this.replay?.playing;
  }

  get speed(): number {
    return this.replay?.speed ?? 1;
  }

  get range(): [number, number] {
    const r = this.replay;
    if (r) return [r.start, r.end];
    const times = world.eventTimes;
    return [times[0] ?? this.wall, this.wall];
  }

  /** The moment every view draws. */
  get viewTime(): number {
    const r = this.replay;
    if (!r) return this.wall;
    if (this.scrubbing != null) return this.scrubbing;
    if (!r.playing) return r.position;
    return Math.min(r.end, r.position + Math.max(0, this.wall - world.statusAt) * r.speed);
  }

  /** Quiet-gap skipping, as the core reports it in replay (ux-spec 2.7). */
  get skipGaps(): boolean {
    return this.replay?.skipGaps ?? this.skipGapsWanted;
  }

  /** Whether the source can compress quiet gaps. */
  get canSkipGaps(): boolean {
    return !!world.source?.setSkipGaps;
  }

  start(): void {
    if (this.started) return;
    this.started = true;
    setInterval(() => {
      if (!this.playing) this.wall = Date.now();
    }, 1000);
    const frame = () => {
      if (this.playing) this.wall = Date.now();
      requestAnimationFrame(frame);
    };
    if (typeof requestAnimationFrame === 'function') requestAnimationFrame(frame);
  }

  /** Re-read the wall clock after a command returned a status. */
  noteStatus(): void {
    this.wall = Date.now();
  }

  private async run(f: () => Promise<unknown> | undefined): Promise<void> {
    try {
      this.lastError = null;
      const st = await f();
      if (st && typeof st === 'object' && 'mode' in st) {
        world.setStatus(st as never);
        this.noteStatus();
      }
    } catch (e) {
      this.lastError = errorText(e);
    }
  }

  play() {
    return this.run(() => world.source?.play());
  }
  pause() {
    return this.run(() => world.source?.pause());
  }
  toggle() {
    return this.playing ? this.pause() : this.play();
  }

  seek(t: number) {
    const [a, b] = this.range;
    const clamped = Math.min(b, Math.max(a, t));
    this.scrubbing = null;
    return this.run(() => world.source?.seek(clamped));
  }

  /** While dragging: show `t` now, send seeks at most every 100 ms. Call `endScrub` on release. */
  scrub(t: number): void {
    const [a, b] = this.range;
    this.scrubbing = Math.min(b, Math.max(a, t));
    const due = SEEK_THROTTLE_MS - (Date.now() - this.lastSeekWall);
    if (this.seekTimer) return;
    this.seekTimer = setTimeout(() => {
      this.seekTimer = null;
      this.lastSeekWall = Date.now();
      const target = this.scrubbing;
      if (target != null) void this.run(() => world.source?.seek(target));
    }, Math.max(0, due));
  }

  endScrub(t: number): Promise<void> {
    if (this.seekTimer) clearTimeout(this.seekTimer);
    this.seekTimer = null;
    return this.seek(t);
  }

  setSpeed(x: number) {
    return this.run(() => world.source?.setSpeed(x));
  }

  /** `[` and `]`: one step down or up the speed list. */
  stepSpeed(dir: 1 | -1) {
    const cur = this.speed;
    const i = SPEEDS.findIndex((s) => s >= cur - 1e-9);
    const idx = i < 0 ? SPEEDS.length - 1 : SPEEDS[i] === cur ? i + dir : dir > 0 ? i : i - 1;
    const next = SPEEDS[Math.max(0, Math.min(SPEEDS.length - 1, idx))];
    return this.setSpeed(next);
  }

  /** `,` and `.`: previous or next known event time; 5 s when nothing is known. */
  step(dir: 1 | -1) {
    const t = this.viewTime;
    const times = this.replay && world.timelineTimes.length ? world.timelineTimes : world.eventTimes;
    let target: number | undefined;
    if (dir > 0) target = times.find((x) => x > t + 1);
    else target = [...times].reverse().find((x) => x < t - 1);
    return this.seek(target ?? t + dir * 5000);
  }

  nudge(ms: number) {
    return this.seek(this.viewTime + ms);
  }

  toStart() {
    return this.seek(this.range[0]);
  }

  toEnd() {
    return this.seek(this.range[1]);
  }

  setSkipGaps(on: boolean) {
    this.skipGapsWanted = on;
    return this.run(() => world.source?.setSkipGaps(on));
  }
}

export const clock = new ClockStore();
