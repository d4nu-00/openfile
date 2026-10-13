import type { EntrySummary, PageResponse } from "../api/types";

/** Rows read per request. */
export const BLOCK = 100;
/** Blocks kept; the least recently wanted are dropped first. */
const MAX_BLOCKS = 60;

type ReadRows = (
  databaseId: string,
  start: string,
  limit: number,
  requestId: string,
) => Promise<PageResponse>;

/**
 * Rows of the game list, read a block at a time. The list can hold
 * millions of rows; only the blocks around what is shown are kept. A new
 * key (another database, search, sort or a save) drops every block, and
 * answers for an older key are ignored.
 */
export class RowCache {
  private blocks = new Map<number, EntrySummary[]>();
  private pending = new Map<number, Promise<void>>();
  private key = "";
  private databaseId = "";
  constructor(
    private readRows: ReadRows,
    private changed: () => void,
    private failed: (message: string) => void = () => {},
  ) {}

  /** Starts over for another list. */
  reset(key: string, databaseId: string) {
    if (key === this.key) return;
    this.key = key;
    this.databaseId = databaseId;
    this.blocks.clear();
    this.pending.clear();
  }

  /** The row at `index` (from 0), if its block is loaded. */
  row(index: number): EntrySummary | undefined {
    return this.blocks.get(Math.floor(index / BLOCK))?.[index % BLOCK];
  }

  /** The row of an entry among the loaded rows. */
  find(ordinal: string): number | undefined {
    for (const [block, rows] of this.blocks) {
      const i = rows.findIndex((r) => r.ordinal === ordinal);
      if (i >= 0) return block * BLOCK + i;
    }
    return undefined;
  }

  /** Loads the blocks covering rows `from .. to` (exclusive) of `rows`. */
  want(from: number, to: number, rows: number) {
    if (!this.databaseId || rows <= 0) return;
    const first = Math.max(0, Math.floor(from / BLOCK));
    const last = Math.min(
      Math.floor((rows - 1) / BLOCK),
      Math.floor(Math.max(from, to - 1) / BLOCK),
    );
    for (let b = first; b <= last; b++) void this.load(b);
  }

  /** The row at `index`, read first when needed. */
  async entry(index: number, rows: number): Promise<EntrySummary | undefined> {
    if (index < 0 || index >= rows) return undefined;
    await this.load(Math.floor(index / BLOCK));
    return this.row(index);
  }

  private load(block: number): Promise<void> {
    const loaded = this.blocks.get(block);
    if (loaded) {
      // Most recently wanted blocks stay longest.
      this.blocks.delete(block);
      this.blocks.set(block, loaded);
      return Promise.resolve();
    }
    const running = this.pending.get(block);
    if (running) return running;
    const key = this.key;
    const request = this.readRows(
      this.databaseId,
      String(block * BLOCK + 1),
      BLOCK,
      `rows-${key}-${block}`,
    )
      .then((page) => {
        if (this.key !== key) return;
        this.blocks.set(block, page.entries);
        while (this.blocks.size > MAX_BLOCKS)
          this.blocks.delete(this.blocks.keys().next().value!);
        this.changed();
      })
      .catch((e: unknown) => {
        if (this.key === key)
          this.failed((e as { message?: string })?.message ?? String(e));
      })
      .finally(() => {
        if (this.key === key) this.pending.delete(block);
      });
    this.pending.set(block, request);
    return request;
  }
}
