let sequence = 0;
export class SelectionGate {
  private current?: string;
  begin(): string {
    this.current = `request-${++sequence}`;
    return this.current;
  }
  accept(requestId: string): boolean {
    return requestId === this.current;
  }
}
