// Streaming and queue state for one chat screen. A class, not a module
// singleton: each tab's ChatScreen owns one, so two tabs never share a
// run. The send loop itself stays in ChatScreen — it needs the whole
// chat — this is the state it runs on, plus the queue operations that
// only touch that state.

export type PendingNote = { id: number; text: string };

export class ChatRunState {
  /** An answer is streaming right now. */
  streaming = $state(false);
  /** The backend job id of the running answer, for cancel. */
  activeJobId = $state<string | null>(null);
  /** A stopped answer waits for Resume. */
  stoppedAnswer = $state(false);
  /** When the running answer started, for the run timeline. */
  runStartedAt = $state<number | null>(null);
  /** Messages written while an answer runs; sent when it ends. */
  queued = $state<PendingNote[]>([]);
  /** Steering notes for the running answer, read at its next step. */
  steering = $state<PendingNote[]>([]);
  private noteSeq = 0;

  /** The queued note being edited inline, if any. */
  editingNoteId = $state<number | null>(null);
  editingNoteText = $state("");

  queueNote(text: string): PendingNote {
    const note = { id: ++this.noteSeq, text };
    this.queued.push(note);
    return note;
  }

  steerNote(text: string): PendingNote {
    const note = { id: ++this.noteSeq, text };
    this.steering.push(note);
    return note;
  }

  /** Take a queued note out to send it now; undefined when it is gone. */
  takeNote(id: number): PendingNote | undefined {
    const at = this.queued.findIndex((n) => n.id === id);
    if (at < 0) return undefined;
    const [note] = this.queued.splice(at, 1);
    return note;
  }

  removeNote(id: number) {
    this.queued = this.queued.filter((n) => n.id !== id);
  }

  /** The next queued message, once the answer ends. */
  shiftNext(): PendingNote | undefined {
    return this.queued.shift();
  }

  /** A note the answer had no step left for joins the queue. */
  moveToQueue(id: number): boolean {
    const at = this.steering.findIndex((n) => n.id === id);
    if (at < 0) return false;
    const [note] = this.steering.splice(at, 1);
    this.queued.push(note);
    return true;
  }

  parkSteers() {
    if (this.steering.length === 0) return;
    this.queued = [...this.steering, ...this.queued];
    this.steering = [];
  }

  moveNote(id: number, dir: -1 | 1) {
    const at = this.queued.findIndex((n) => n.id === id);
    const to = at + dir;
    if (at < 0 || to < 0 || to >= this.queued.length) return;
    const [note] = this.queued.splice(at, 1);
    this.queued.splice(to, 0, note);
  }

  startNoteEdit(id: number) {
    const note = this.queued.find((n) => n.id === id);
    if (!note) return;
    this.editingNoteId = id;
    this.editingNoteText = note.text;
  }

  saveNoteEdit() {
    if (this.editingNoteId === null) return;
    const text = this.editingNoteText.trim();
    if (!text) return;
    const note = this.queued.find((n) => n.id === this.editingNoteId);
    if (note) note.text = text;
    this.editingNoteId = null;
    this.editingNoteText = "";
  }

  cancelNoteEdit() {
    this.editingNoteId = null;
    this.editingNoteText = "";
  }
}
