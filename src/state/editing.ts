import { useEffect, useRef, useState } from "react";
import type { WorkbenchApi } from "../api/client";
import type { Edit, PgnCollectionResponse, GameResponse } from "../api/types";
import type { useWorkbench } from "./workbench";
type Workbench = ReturnType<typeof useWorkbench>;
export interface Question {
  kind: "unsaved" | "compatibility";
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
    if (!d?.documentId || !d.game) return false;
    try {
      const incomplete = d.annotationsStatus === "notDecoded";
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
    }
  }
  async function guard(): Promise<boolean> {
    if (editBusy.current) return false;
    if (!w.current.current.dirty) return true;
    const value = await ask("unsaved");
    if (value === "save") return (await save()) && !w.current.current.dirty;
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
  async function edit(value: Edit): Promise<number | undefined> {
    if (transition.current || editBusy.current || pending.current) return;
    const d = w.current.current.document;
    if (!d?.documentId) return;
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
      const id =
        value.kind === "appendMove"
          ? result.game.nodes[value.parent].children.find(
              (n) => result.game.nodes[n].uci === value.uci,
            )
          : value.node;
      if (value.kind === "appendMove" && id !== undefined)
        await w.selectNode(id);
      return id;
    } catch (e) {
      report(e);
    } finally {
      editBusy.current = false;
      setBusy(transition.current);
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
      w.setState((s) => ({ ...s, document: doc, position, dirty: false }));
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
    guard,
    run,
    edit,
    collection,
    openPgn,
    loadPgn,
    closeDocument,
  };
}
