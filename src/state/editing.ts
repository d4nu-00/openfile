import { useEffect, useRef, useState } from "react";
import type { WorkbenchApi } from "../api/client";
import {
  annotationsIncomplete,
  structuralEdits,
  type DocumentResponse,
  type Edit,
  type PgnCollectionResponse,
  type GameResponse,
} from "../api/types";
import type { useWorkbench } from "./workbench";
type Workbench = ReturnType<typeof useWorkbench>;
export interface Question {
  kind: "unsaved" | "compatibility" | "database";
  resolve: (answer: string) => void;
}
export function useEditing(w: Workbench, api: WorkbenchApi) {
  const [question, setQuestion] = useState<Question>();
  const pending = useRef<Question | undefined>(undefined);
  const [busy, setBusy] = useState(false);
  const sequence = useRef(0);
  const transition = useRef(false);
  const [collection, setCollection] = useState<PgnCollectionResponse>();
  const collectionRef = useRef<PgnCollectionResponse | undefined>(undefined);
  const gate = w.gates.game;
  const editBusy = useRef(false);
  const saveBusy = useRef(false);
  const report = (e: unknown) =>
    w.setDiagnostics([
      {
        code: "document",
        message: (e as { message?: string })?.message ?? String(e),
      },
    ]);
  function ask(kind: Question["kind"]): Promise<string> {
    if (pending.current) return Promise.resolve("cancel");
    return new Promise((resolve) => {
      const q = { kind, resolve };
      pending.current = q;
      setQuestion(q);
    });
  }
  function answer(value: string) {
    const q = pending.current;
    pending.current = undefined;
    setQuestion(undefined);
    q?.resolve(value);
  }
  async function save(): Promise<boolean> {
    const d = w.current.current.document;
    if (!d?.documentId || !d.game || saveBusy.current) return false;
    saveBusy.current = true;
    setBusy(true);
    try {
      const incomplete = annotationsIncomplete(d.annotationsStatus);
      if (incomplete && (await ask("compatibility")) !== "export") return false;
      const path = await api.chooseSave();
      if (typeof path !== "string") return false;
      const result = await api.saveDocument(
        d.documentId,
        path,
        d.revision ?? "0",
        incomplete,
      );
      if (
        w.current.current.document?.documentId === d.documentId &&
        w.current.current.document?.revision === result.savedRevision
      ) {
        w.setState((s) => ({
          ...s,
          dirty: false,
          document: s.document && { ...s.document, dirty: false },
        }));
        w.setStatus(`Saved ${result.destination}`);
        return true;
      }
      return false;
    } catch (e) {
      report(e);
      return false;
    } finally {
      saveBusy.current = false;
      setBusy(transition.current || editBusy.current);
    }
  }
  /**
   * Saves the game into a database opened for editing: as a new entry
   * (ChessBase "Save"), or over the entry it came from ("Replace").
   */
  async function saveToDatabase(mode: "save" | "replace"): Promise<boolean> {
    const d = w.current.current.document;
    const db = w.current.current.database;
    const origin = w.current.current.origin;
    if (!d?.documentId || !d.game || saveBusy.current) return false;
    const target = mode === "replace" ? origin?.databaseId : db?.databaseId;
    if (!target || (mode === "save" && !db?.writable)) {
      report(
        new Error(
          mode === "replace"
            ? "This game was not loaded from a database opened for editing"
            : "Create a database or open one for editing first",
        ),
      );
      return false;
    }
    saveBusy.current = true;
    setBusy(true);
    try {
      let acknowledged = annotationsIncomplete(d.annotationsStatus);
      if (acknowledged && (await ask("database")) !== "save") return false;
      const attempt = () =>
        mode === "save"
          ? api.saveGame(d.documentId!, target, d.revision ?? "0", acknowledged)
          : api.replaceGame(d.documentId!, d.revision ?? "0", acknowledged);
      let result;
      try {
        result = await attempt();
      } catch (e) {
        // Content ChessBase cannot store is found only while encoding.
        const code = (e as { code?: string })?.code;
        if (code !== "incompleteAnnotations" || acknowledged) throw e;
        if ((await ask("database")) !== "save") return false;
        acknowledged = true;
        result = await attempt();
      }
      if (w.current.current.document?.documentId === d.documentId) {
        const clean =
          w.current.current.document?.revision === result.savedRevision;
        w.setState((s) => ({
          ...s,
          dirty: clean ? false : s.dirty,
          origin: { databaseId: result.databaseId, ordinal: result.ordinal },
          document: s.document && {
            ...s.document,
            dirty: clean ? false : s.document.dirty,
            summary: { ...s.document.summary, ordinal: result.ordinal },
            annotationsStatus: acknowledged
              ? "available"
              : s.document.annotationsStatus,
          },
          database:
            s.database?.databaseId === result.databaseId
              ? { ...s.database, entryCount: result.entryCount }
              : s.database,
        }));
      }
      if (w.current.current.database?.databaseId === result.databaseId)
        await w.refreshList();
      w.setStatus(
        `${mode === "save" ? "Saved as" : "Replaced"} game ${result.ordinal}`,
      );
      return true;
    } catch (e) {
      report(e);
      return false;
    } finally {
      saveBusy.current = false;
      setBusy(transition.current || editBusy.current);
    }
  }
  /** Marks the selected entry deleted, or undeletes it (ChessBase "Delete"). */
  async function toggleDeleted(ordinal: string) {
    const db = w.current.current.database;
    const at = w.rows.find(ordinal);
    const entry = at === undefined ? undefined : w.rows.row(at);
    if (!db?.writable || !entry || entry.kind !== "game") return;
    try {
      await api.setDeleted(db.databaseId, ordinal, !entry.deleted);
      await w.refreshList();
      w.setStatus(
        `Game ${ordinal} ${entry.deleted ? "undeleted" : "marked deleted"}`,
      );
    } catch (e) {
      report(e);
    }
  }
  /**
   * The save behind the unsaved-changes dialog: replace the entry the game
   * came from when its database is open for editing, otherwise save into the
   * editable database, otherwise export PGN.
   */
  function saveDefault(): Promise<boolean> {
    const { origin, database } = w.current.current;
    if (database?.writable && origin?.databaseId === database.databaseId)
      return saveToDatabase("replace");
    if (database?.writable) return saveToDatabase("save");
    return save();
  }
  async function guard(): Promise<boolean> {
    if (editBusy.current || saveBusy.current) return false;
    if (!w.current.current.dirty) return true;
    const value = await ask("unsaved");
    if (value === "save")
      return (await saveDefault()) && !w.current.current.dirty;
    return value === "discard";
  }
  async function run(action: () => Promise<void>) {
    if (!(await guard())) return;
    const ticket = ++sequence.current;
    transition.current = true;
    setBusy(true);
    // A newly approved replacement supersedes earlier pending replacements.
    w.gates.database.begin();
    w.gates.game.begin();
    w.gates.position.begin();
    try {
      await action();
    } catch (e) {
      report(e);
    } finally {
      if (ticket === sequence.current) {
        transition.current = false;
        setBusy(editBusy.current);
      }
    }
  }
  /** Applies an edit; resolves to the node to select, or null on failure. */
  async function edit(value: Edit): Promise<number | undefined | null> {
    if (transition.current || editBusy.current || pending.current) return null;
    const d = w.current.current.document;
    if (!d?.documentId) return null;
    editBusy.current = true;
    setBusy(true);
    try {
      const result = await api.editDocument(
        d.documentId,
        value,
        d.revision ?? "0",
      );
      if (w.current.current.document?.documentId !== d.documentId) return;
      w.setState((s) => ({
        ...s,
        document: s.document && { ...s.document, ...result },
        dirty: result.dirty,
      }));
      if (structuralEdits.includes(value.kind)) {
        // Ids may have changed: select what the edit chose, or keep the
        // current node while it exists.
        const current = w.current.current.position?.nodeId ?? 0;
        const id =
          result.selected ?? (current < result.game.nodes.length ? current : 0);
        await w.selectNode(id);
        return id;
      }
      const id =
        value.kind === "appendMove"
          ? result.game.nodes[value.parent].children.find(
              (n) => result.game.nodes[n].uci === value.uci,
            )
          : "node" in value
            ? value.node
            : undefined;
      if (value.kind === "appendMove" && id !== undefined)
        await w.selectNode(id);
      return id;
    } catch (e) {
      report(e);
      return null;
    } finally {
      editBusy.current = false;
      setBusy(transition.current);
    }
  }
  /** Shows a new game in the game tab: empty, set up, or pasted. */
  async function openDocument(
    make: (request: string) => Promise<DocumentResponse>,
    status: string,
  ) {
    const request = gate.begin();
    let candidate: string | undefined;
    try {
      const made = await make(request);
      candidate = made.documentId;
      const position = await api.positionAt(made.documentId, 0, request);
      if (!gate.accept(request)) {
        void api.closeDocument(candidate).catch(() => {});
        return;
      }
      const old = w.current.current.document?.documentId;
      const doc: GameResponse = {
        ...made,
        requestId: request,
        summary: { ordinal: "", kind: "game", deleted: false, diagnostics: [] },
        metadataStatus: "available",
        movesStatus: "available",
        annotationsStatus: "available",
        diagnostics: [],
      };
      w.setState((s) => ({
        ...s,
        document: doc,
        position,
        dirty: false,
        origin: undefined,
      }));
      if (old && old !== candidate) void api.closeDocument(old).catch(() => {});
      w.setDiagnostics([]);
      w.setStatus(status);
    } catch (e) {
      if (candidate) void api.closeDocument(candidate).catch(() => {});
      if (gate.accept(request)) report(e);
      throw e;
    }
  }
  /** A new game from the initial position, or from a set-up position. */
  const newGame = (fen?: string) =>
    openDocument(
      (request) => api.newDocument(fen ?? null, request),
      fen ? "New game from a set-up position" : "New game",
    );
  const pasteGame = (text: string) =>
    openDocument((request) => api.pasteGame(text, request), "Pasted game");
  /** Copies the game as PGN. */
  async function copyGame() {
    const d = w.current.current.document;
    if (!d?.documentId) return;
    try {
      await api.writeClipboard(await api.documentPgn(d.documentId));
      w.setStatus("Game copied as PGN");
    } catch (e) {
      report(e);
    }
  }
  const closeCollection = (id?: string) => {
    if (id) void api.closePgn(id).catch(() => {});
  };
  async function loadPgn(index: number, c = collectionRef.current) {
    if (!c) return;
    const request = gate.begin();
    let candidate: string | undefined;
    try {
      const loaded = await api.loadPgnGame(c.collectionId, index, request);
      candidate = loaded.documentId;
      const position = await api.positionAt(loaded.documentId, 0, request);
      if (!gate.accept(request)) {
        void api.closeDocument(candidate).catch(() => {});
        return;
      }
      const old = w.current.current.document?.documentId;
      const doc: GameResponse = {
        ...loaded,
        requestId: request,
        summary: {
          ordinal: String(index + 1),
          kind: "game",
          deleted: false,
          diagnostics: [],
        },
        metadataStatus: "available",
        movesStatus: "available",
        annotationsStatus: "available",
        diagnostics: [],
      };
      w.setState((s) => ({
        ...s,
        document: doc,
        position,
        dirty: false,
        origin: undefined,
      }));
      if (old && old !== candidate) void api.closeDocument(old).catch(() => {});
      closeCollection(
        collectionRef.current?.collectionId === c.collectionId
          ? undefined
          : collectionRef.current?.collectionId,
      );
      collectionRef.current = c;
      setCollection(c);
      w.setDiagnostics([]);
      w.setStatus(`PGN game ${index + 1} of ${c.gameCount}`);
    } catch (e) {
      if (candidate) void api.closeDocument(candidate).catch(() => {});
      if (gate.accept(request)) report(e);
      throw e;
    }
  }
  async function openPgn() {
    let candidate: PgnCollectionResponse | undefined;
    try {
      const path = await api.choosePgn();
      if (typeof path !== "string") return;
      w.gates.database.begin();
      const request = gate.begin();
      candidate = await api.openPgn(path, request);
      if (!gate.accept(request)) {
        closeCollection(candidate.collectionId);
        return;
      }
      if (!candidate.gameCount) {
        closeCollection(candidate.collectionId);
        throw { message: "PGN contains no games" };
      }
      await loadPgn(0, candidate);
    } catch (e) {
      if (
        candidate &&
        collectionRef.current?.collectionId !== candidate.collectionId
      )
        closeCollection(candidate.collectionId);
      report(e);
    }
  }
  async function closeDocument() {
    gate.begin();
    const id = w.current.current.document?.documentId;
    if (id) await api.closeDocument(id);
    w.setState((s) => ({
      ...s,
      document: undefined,
      position: undefined,
      origin: undefined,
      dirty: false,
    }));
  }
  const closeHandler = useRef(() => {});
  closeHandler.current = () => {
    void run(async () => {
      await api.finishClose();
    });
  };
  useEffect(() => {
    let disposed = false;
    let off: (() => void) | undefined;
    void api
      .watchClose(() => closeHandler.current())
      .then((unlisten) => {
        if (disposed) unlisten();
        else off = unlisten;
      })
      .catch(report);
    return () => {
      disposed = true;
      off?.();
    };
  }, [api]);
  useEffect(
    () => () => {
      gate.begin();
      pending.current?.resolve("cancel");
      closeCollection(collectionRef.current?.collectionId);
    },
    [api],
  );
  return {
    question,
    answer,
    busy,
    save,
    saveToDatabase,
    toggleDeleted,
    guard,
    run,
    edit,
    collection,
    openPgn,
    loadPgn,
    closeDocument,
    newGame,
    pasteGame,
    copyGame,
  };
}
