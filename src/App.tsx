import { useEffect, useMemo, useRef, useState } from "react";
import { Group, Panel, Separator } from "react-resizable-panels";
import { Database, LayoutGrid, Search, X } from "lucide-react";
import { backend, type WorkbenchApi } from "./api/client";
import {
  annotationsIncomplete,
  defaultInstant,
  defaultSettings,
  emptyFilter,
  type AppSettings,
  type BlunderParams,
  type CentipawnReport,
  type NoveltyReport,
  type Classification,
  type GameView,
  type RepertoireGrade,
  type WorkspaceSide,
  type OpeningReport,
  type DeepParams,
  type DeepProgress,
  type JobKind,
  type JobsView,
  type RunJobsParams,
} from "./api/types";
import type {
  CrossLayout,
  Crosstable,
  NameKind,
  NameRow,
  ConvertEstimate,
  DoubleOptions,
  SelectionNames,
  DatabaseSymbol,
  Edit,
  PlayerStatistics,
  Preparation,
  Dossier,
  TournamentReport,
  ErrorReport,
  GameNode,
  SelectionPlayer,
  SortColumn,
} from "./api/types";
import {
  addTab,
  tabGames,
  useWorkbench,
  type WorkbenchState,
} from "./state/workbench";
import { OptionsDialog } from "./components/OptionsDialog";
import { DatabasePropertiesDialog } from "./components/DatabasePropertiesDialog";
import { RecoveryDialog } from "./components/RecoveryDialog";
import { useEditing } from "./state/editing";
import { formatScore } from "./state/analysis";
import { useKibitzers } from "./state/kibitzers";
import { mainLine, useInstantAnalysis } from "./state/evalProfile";
import { EvalProfile } from "./components/EvalProfile";
import { CentipawnListDialog } from "./components/CentipawnListDialog";
import { MedalDialog, MoveTimeDialog } from "./components/MoveExtrasDialogs";
import { NoveltyDialog } from "./components/NoveltyDialog";
import { plyName } from "./components/EvalProfile";
import { boardMarks } from "./state/moveExtras";
import { EngineManagement } from "./components/EngineManagement";
import { TablebasePane } from "./components/TablebasePane";
import { BlunderCheckDialog } from "./components/BlunderCheckDialog";
import { CentipawnDialog } from "./components/CentipawnDialog";
import { TrainingDialog } from "./components/TrainingDialog";
import {
  ReplayTrainingSession,
  type ReplaySource,
} from "./components/ReplayTrainingSession";
import { DeepAnalysisDialog } from "./components/DeepAnalysisDialog";
import { ClassifyDialog, SetEcoDialog } from "./components/OpeningDialogs";
import { OpeningReportDialog } from "./components/OpeningReportDialog";
import { ReorderDialog } from "./components/ReorderDialog";
import {
  AnalysisJobsDialog,
  defaultJobKind,
} from "./components/AnalysisJobsDialog";
import { CommentEditor } from "./components/CommentEditor";
import { GameDataDialog } from "./components/GameDataDialog";
import { PositionSetup } from "./components/PositionSetup";
import { PasteDialog } from "./components/PasteDialog";
import { AnnotationPalette } from "./components/AnnotationPalette";
import { SymbolMenu } from "./components/SymbolMenu";
import { ConvertDialog, type ConvertScope } from "./components/ConvertDialog";
import { DoublesDialog, JobDialog } from "./components/MaintenanceDialogs";
import { SelectionDataDialog } from "./components/SelectionDataDialog";
import type { SymbolSlot } from "./state/symbols";
import { UnsavedDialog } from "./components/UnsavedDialog";
import { SearchMask } from "./components/SearchMask";
import { EnginePanel } from "./components/EnginePanel";
import { NotationTree } from "./components/NotationTree";
import { BookPane } from "./components/BookPane";
import { ReferencePane } from "./components/ReferencePane";
import { RepertoirePane } from "./components/RepertoirePane";
import { lineTo, usePositionPanes } from "./state/positionPanes";
import { Diagnostics } from "./components/Diagnostics";
import { GameList } from "./views/GameList";
import { NamesPane } from "./views/NamesPane";
import { StartScreen, type StartView } from "./views/StartScreen";
import { CrosstableDialog } from "./components/CrosstableDialog";
import { PlayerStatisticsDialog } from "./components/PlayerStatisticsDialog";
import { PrepareDialog } from "./components/PrepareDialog";
import {
  GenerateRepertoireDialog,
  RepertoireReportDialog,
} from "./components/RepertoireDialogs";
import { PlayingStyleDialog } from "./components/PlayingStyleDialog";
import { DossierDialog } from "./components/DossierDialog";
import { TournamentReportDialog } from "./components/TournamentReportDialog";
import { ErrorReportDialog } from "./components/ErrorReportDialog";
import {
  chooseRow,
  isSelected,
  noSelection,
  rowRanges,
  selectAll,
  selectedCount,
  type Selection,
} from "./state/listSelection";
import { BoardPane, type Step } from "./views/BoardPane";
import type { DrawnMark } from "./components/Board";
import { toggleMark } from "./state/marks";
import { Ribbon } from "./shell/Ribbon";
import { CommandPalette } from "./shell/CommandPalette";
import { useShortcuts, type Bindings } from "./commands/bindings";
import type { ViewId } from "./commands/catalogue";
import { format } from "./commands/keys";
import "./styles/theme.css";
import "./styles/shell.css";
import "./styles/board.css";

/** The standard start position. */
const START_FEN = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

export type Theme = "white" | "blue" | "gray" | "black";
type Layout = "standard" | "wide-list" | "big-board" | "big-engine";
const themeKey = "workbench.theme";
function storedTheme(): Theme {
  try {
    const t = localStorage.getItem(themeKey);
    if (t === "white" || t === "blue" || t === "gray" || t === "black")
      return t;
  } catch {
    // Storage may be unavailable; the default theme applies.
  }
  return "white";
}

function mainLineKey(game?: GameView): string {
  if (!game) return "";
  const moves: string[] = [game.startFen];
  let node = 0;
  for (let i = 0; i < game.nodes.length; i++) {
    const next = game.nodes[node]?.children[0];
    if (next === undefined || !game.nodes[next]) break;
    moves.push(game.nodes[next].uci ?? "");
    node = next;
  }
  return moves.join(" ");
}

export function App({
  api = backend,
  initialState,
  initialView = "database",
}: {
  api?: WorkbenchApi;
  initialState?: WorkbenchState;
  initialView?: ViewId;
}) {
  // A game given for the game view starts in a tab of its own.
  const [start] = useState(() =>
    initialView === "game" &&
    initialState?.document &&
    !initialState.tabs?.length
      ? addTab(
          { ...initialState, document: undefined },
          {
            document: initialState.document,
            position: initialState.position,
            dirty: initialState.dirty,
            origin: initialState.origin,
          },
        )
      : initialState,
  );
  const w = useWorkbench(api, start);
  const { state } = w;
  /** The workbench's settings, from its config folder. */
  const [settings, setSettings] = useState<AppSettings>(defaultSettings);
  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  useEffect(() => {
    void api
      .getSettings()
      .then((s) => {
        setSettings(s);
        setStartView(s.databaseView ?? "symbols");
      })
      .catch(() => {});
  }, [api]);
  const [optionsOpen, setOptionsOpen] = useState(false);
  const [optionsError, setOptionsError] = useState<string>();
  const e = useEditing(w, api, () => settingsRef.current.maxGameTabs);
  /** Saves settings to the config folder and shows them. */
  async function saveSettings(next: AppSettings) {
    setSettings(await api.setSettings(next));
  }
  const k = useKibitzers(api, state, settings, saveSettings);
  const [enginesOpen, setEnginesOpen] = useState(false);
  /** Tactical Analysis: open, its progress while running, its error. */
  /** The move whose training question is being edited, and any error. */
  const [training, setTraining] = useState<{ node: number; error?: string }>();
  /** Find Novelty: running, then its answer. */
  const [novelty, setNovelty] = useState<{
    documentId: string;
    running: boolean;
    report?: NoveltyReport;
    error?: string;
  }>();
  /** Set Medal or Move Time for a move. */
  const [extraDialog, setExtraDialog] = useState<{
    kind: "medal" | "moveTime";
    node: number;
  }>();
  const [replay, setReplay] = useState<{
    source: ReplaySource;
    databaseId?: string;
  }>();
  /** The document Centipawn Analysis runs on. */
  const [centipawn, setCentipawn] = useState<string>();
  /** Centipawn Analysis of the chosen list rows. */
  const [centipawnList, setCentipawnList] = useState<{
    databaseId: string;
    name: string;
    ranges: { start: string; end: string }[];
    selected: number;
  }>();
  const [gameReview, setGameReview] = useState<{
    documentId: string;
    line: string;
    report: CentipawnReport;
  }>();
  const [blunder, setBlunder] = useState<{
    running?: string;
    error?: string;
    documentId?: string;
  }>();
  const [deep, setDeep] = useState<{
    running?: DeepProgress | true;
    error?: string;
    documentId?: string;
  }>();
  const [jobs, setJobs] = useState<{
    view: JobsView;
    progress?: string;
    error?: string;
  }>();
  const [jobKind, setJobKind] = useState<JobKind>(defaultJobKind);
  const [enginesError, setEnginesError] = useState<string>();
  /** The game view shows a game tab; the database view, the preview. */
  const view: ViewId = state.focus !== undefined ? "game" : "database";
  function setView(next: ViewId) {
    if (next === "database") return w.focusGame(undefined);
    if (w.current.current.focus !== undefined) return;
    const last = [...(w.current.current.tabs ?? [])].sort(
      (x, y) => y.used - x.used,
    )[0];
    if (last) w.focusGame(last.id);
    else void e.openPreview();
  }
  const [theme, setTheme] = useState<Theme>(storedTheme);
  const [layout, setLayout] = useState<Layout>("standard");
  const [palette, setPalette] = useState(false);
  const [filter, setFilter] = useState("");
  const [filterOpen, setFilterOpen] = useState(false);
  /** The search mask, and the tab it opens on ("annotations" for Search
   * text). */
  const [maskOpen, setMaskOpen] = useState<false | true | "annotations">(false);
  /** The database window's list: games, players or tournaments. */
  const [listTab, setListTab] = useState<"games" | NameKind>("games");
  const [openedTabs, setOpenedTabs] = useState<NameKind[]>([]);
  const [chosenName, setChosenName] = useState<
    Partial<Record<NameKind, NameRow>>
  >({});
  const [cross, setCross] = useState<{
    tournamentId: number;
    table?: Crosstable;
    loading?: boolean;
    error?: string;
  }>();
  /** The player statistics shown, and the players to switch between. */
  const [stats, setStats] = useState<{
    databaseId: string;
    playerId: number;
    data?: PlayerStatistics;
    progress?: string;
    error?: string;
    players?: { id: number; name?: string }[];
  }>();
  /** Prepare against White or Black: the dialog's player and result. */
  /** Generate Repertoire, or a Repertoire Report of one colour. */
  const [repertoire, setRepertoire] = useState<
    { databaseId: string; report?: { white: boolean } } | undefined
  >();
  /** Repertoire databases generated or chosen in this session. */
  const repertoirePaths = useRef<{ white?: string; black?: string }>({});
  const [prep, setPrep] = useState<{
    databaseId: string;
    playerId: number;
    white: boolean;
    name?: string;
    data?: Preparation;
    progress?: string;
    error?: string;
  }>();
  /** The Error Report: its selection, players, progress and result. */
  const [errors, setErrors] = useState<{
    databaseId: string;
    ranges: { start: string; end: string }[];
    selected: number;
    players: SelectionPlayer[];
    report?: ErrorReport;
    running?: string;
    error?: string;
  }>();
  /** The tournament report shown. */
  const [tourReport, setTourReport] = useState<{
    tournamentId: number;
    data?: TournamentReport;
    loading?: boolean;
    error?: string;
  }>();
  const [playingStyle, setPlayingStyle] = useState<{
    databaseId: string;
    databaseName: string;
    player: { id: number; name?: string };
  }>();
  useEffect(() => {
    if (playingStyle && playingStyle.databaseId !== state.database?.databaseId)
      setPlayingStyle(undefined);
  }, [state.database?.databaseId, playingStyle]);
  /** The player dossier shown. */
  const [dossier, setDossier] = useState<{
    databaseId: string;
    playerId: number;
    name?: string;
    data?: Dossier;
    progress?: string;
    error?: string;
  }>();
  /** The preparation the Book pane shows instead of the opening tree. */
  const [bookPrep, setBookPrep] = useState<{
    databaseId: string;
    playerId: number;
    white: boolean;
    label: string;
  }>();
  const [bookScope, setBookScope] = useState<"database" | "search">("database");
  /** The start screen (My Databases): shown when asked, or with no database. */
  const [startOpen, setStartOpen] = useState(false);
  const [showFolders, setShowFolders] = useState(true);
  const [startView, setStartView] = useState<StartView>("symbols");
  const [startSymbol, setStartSymbol] = useState<DatabaseSymbol>();
  const [startRefresh, setStartRefresh] = useState(0);
  const [propertySymbol, setPropertySymbol] = useState<DatabaseSymbol>();
  const [recovery, setRecovery] = useState<{ source?: string }>();
  const viewSaves = useRef(Promise.resolve());
  const changeStartView = (view: StartView) => {
    setStartView(view);
    viewSaves.current = viewSaves.current
      .then(() => api.setDatabaseView(view))
      .catch((err: unknown) => {
        w.setDiagnostics([
          {
            code: "settings",
            message: (err as { message?: string })?.message ?? String(err),
          },
        ]);
      });
  };
  const [flipped, setFlipped] = useState(false);
  const [ordinal, setOrdinal] = useState("1");
  const [square, setSquare] = useState<string>();
  const [promotion, setPromotion] = useState<string[]>([]);
  const [commentFocus, setCommentFocus] = useState(0);
  const [focusField, setFocusField] = useState<"after" | "before">("after");
  const [gameDataOpen, setGameDataOpen] = useState(false);
  const [setupOpen, setSetupOpen] = useState(false);
  const [setupError, setSetupError] = useState<string>();
  const [pasting, setPasting] = useState<"game" | "position">();
  const [paletteOpen, setPaletteOpen] = useState(false);
  /** Folded variation nodes, per game document (the reference app's Fold). */
  const [folds, setFolds] = useState<Record<string, number[]>>({});
  /** The symbol popup opened by typing !, ?, = or $. */
  const [symbolMenu, setSymbolMenu] = useState<SymbolSlot>();
  // Notation tabs, and the book and reference panes outside them.
  const [notationTab, setNotationTab] = useState<
    "notation" | "book" | "reference" | "repertoire" | "tablebase"
  >("notation");
  const [workspaceSide, setWorkspaceSide] = useState<WorkspaceSide>("white");
  const [practice, setPractice] = useState<{
    lineKey: string;
    side: WorkspaceSide;
    result?: RepertoireGrade;
  }>();
  const [repertoireRefresh, setRepertoireRefresh] = useState(0);
  const [extraBook, setExtraBook] = useState(false);
  const [extraReference, setExtraReference] = useState(false);

  const game = state.document?.game;
  const selected = state.position?.nodeId ?? 0;
  const foldKey = state.document?.documentId ?? "";
  const folded = useMemo(() => new Set(folds[foldKey] ?? []), [folds, foldKey]);
  const setFolded = (ids: number[]) =>
    setFolds((f) => ({ ...f, [foldKey]: ids }));
  const setFold = (id: number, on: boolean) =>
    setFolds((f) => {
      const now = new Set(f[foldKey] ?? []);
      if (on) now.add(id);
      else now.delete(id);
      return { ...f, [foldKey]: [...now] };
    });
  /** The first branch node from the selected move's own branch onwards,
   * along the line, that `wanted` accepts (Fold/Unfold next node). */
  function nextBranch(wanted: (id: number) => boolean): number | undefined {
    if (!game) return undefined;
    let id: number | undefined = game.nodes[selected]?.parent ?? 0;
    for (let i = 0; id !== undefined && i < game.nodes.length; i++) {
      const n: (typeof game.nodes)[number] | undefined = game.nodes[id];
      if (!n) return undefined;
      if (n.children.length > 1 && wanted(id)) return id;
      id = n.children[0];
    }
    return undefined;
  }
  const inGame = view === "game";
  // Instant Analysis follows the game tab shown, not the preview.
  const instant = useInstantAnalysis(
    api,
    settings,
    inGame ? state.document?.documentId : undefined,
    inGame ? game : undefined,
  );
  const lineIds = useMemo(() => mainLine(game), [game]);
  /** The evaluation profile shown: instant analysis's, else the game's own. */
  function profileView(compact: boolean) {
    if (!game) return null;
    const ran = instant.state;
    const points = ran?.progress?.profile.length
      ? ran.progress.profile
      : (game.profile ?? []);
    const current = lineIds.indexOf(selected);
    const p = ran?.progress;
    const status = ran ? (
      <span className="eval-profile-status" role="status">
        {ran.error ??
          (p
            ? `Instant analysis: pass ${p.pass}, ${p.done}/${p.total} positions at ${p.millisPerPosition} ms`
            : "Instant analysis starting")}
        {ran.running ? (
          <button type="button" onClick={() => instant.stop()}>
            Stop
          </button>
        ) : (
          !ran.error &&
          !p?.finished && (
            <button type="button" onClick={() => instant.restart()}>
              Restart
            </button>
          )
        )}
      </span>
    ) : game.profile?.length ? (
      <span className="eval-profile-status">Profile stored with the game</span>
    ) : undefined;
    return (
      <EvalProfile
        points={points}
        plies={lineIds.length - 1}
        current={current >= 0 ? current : undefined}
        san={(ply) => game.nodes[lineIds[ply]]?.san ?? ""}
        whiteFirst={game.startFen.split(" ")[1] !== "b"}
        compact={compact}
        onSelect={(ply) => {
          const id = lineIds[ply];
          if (id !== undefined) void w.selectNode(id);
        }}
        status={
          <>
            {status}
            {!compact && settings.instantAnalysis?.enabled && (
              <span className="instant-options">
                <label>
                  Time{" "}
                  <select
                    aria-label="Instant analysis time"
                    value={settings.instantAnalysis.level}
                    onChange={(ev) =>
                      void saveSettings({
                        ...settings,
                        instantAnalysis: {
                          ...settings.instantAnalysis!,
                          level: Number(ev.target.value),
                        },
                      })
                    }
                  >
                    {[1, 2, 3, 4, 5, 6].map((n) => (
                      <option key={n} value={n}>
                        {n * 35} ms
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  <input
                    type="checkbox"
                    checked={settings.instantAnalysis.endless}
                    onChange={(ev) =>
                      void saveSettings({
                        ...settings,
                        instantAnalysis: {
                          ...settings.instantAnalysis!,
                          endless: ev.target.checked,
                        },
                      })
                    }
                  />{" "}
                  Endless
                </label>
              </span>
            )}
          </>
        }
      />
    );
  }
  const bookPrepared =
    bookPrep && bookPrep.databaseId === state.database?.databaseId
      ? bookPrep
      : undefined;
  const panes = usePositionPanes(
    api,
    state,
    {
      book:
        inGame &&
        (notationTab === "book" || notationTab === "repertoire" || extraBook),
      reference: inGame && (notationTab === "reference" || extraReference),
    },
    notationTab !== "repertoire" && bookPrepared
      ? `${bookPrepared.playerId}:${bookPrepared.white}`
      : undefined,
    notationTab === "repertoire" ? "database" : bookScope,
  );
  const currentLineKey = panes.line
    ? `${panes.line.startFen}|${panes.line.moves.join(" ")}`
    : "";
  const activePractice =
    notationTab === "repertoire" &&
    practice?.lineKey === currentLineKey &&
    practice.side === workspaceSide
      ? practice
      : undefined;
  const activePracticeRef = useRef(activePractice);
  activePracticeRef.current = activePractice;
  const gradingRef = useRef<typeof activePractice>(undefined);
  useEffect(() => {
    if (notationTab !== "repertoire") setPractice(undefined);
  }, [notationTab]);
  const title = game
    ? `${game.tags.White ?? "?"} — ${game.tags.Black ?? "?"}`
    : "Game board";

  useEffect(() => {
    setSquare(undefined);
    setPromotion([]);
  }, [state.position]);
  useEffect(() => {
    try {
      localStorage.setItem(themeKey, theme);
    } catch {
      // Theme persistence is a convenience only.
    }
    document.documentElement.dataset.theme = theme;
  }, [theme]);
  // A database named on the command line opens at start, optionally at an
  // entry; debug builds may also run one command before the snapshot.
  const [startupCommand, setStartupCommand] = useState<string>();
  useEffect(() => {
    let live = true;
    void (async () => {
      const startup = await api.startupRequest().catch(() => null);
      if (!live || !startup) return;
      if (!startup.path && !startup.fen) {
        // Nothing to open: the start screen is what a snapshot shows.
        if (startup.command) setStartupCommand(startup.command);
        else await api.startupShown().catch(() => {});
        return;
      }
      if (startup.path) await w.openDatabase(startup.path);
      if (startup.sort) {
        const [column, order] = startup.sort.split(":");
        await w.sortBy(column as SortColumn, order === "desc");
      }
      if (startup.entry) {
        await w.goToEntry(startup.entry);
        setOrdinal(startup.entry);
        await e.openPreview();
        if (startup.node != null) await w.selectNode(startup.node);
      }
      if (startup.fen) await e.newGame(startup.fen);
      if (startup.command) setStartupCommand(startup.command);
      else await api.startupShown().catch(() => {});
    })();
    return () => {
      live = false;
    };
  }, [api]);

  function clickSquare(to: string, from = square) {
    if (e.busy || !game) return;
    const moves =
      state.position?.legalUci.filter(
        (m) => m.slice(0, 2) === from && m.slice(2, 4) === to,
      ) ?? [];
    if (moves.length > 1) {
      setPromotion(moves);
      setSquare(undefined);
      return;
    }
    if (moves.length === 1) {
      if (activePractice) void gradePractice(moves[0]);
      else void e.edit({ kind: "appendMove", parent: selected, uci: moves[0] });
      setSquare(undefined);
      return;
    }
    setSquare(
      state.position?.legalUci.some((m) => m.slice(0, 2) === to)
        ? to
        : undefined,
    );
  }
  async function gradePractice(uci: string) {
    if (
      !activePractice ||
      activePractice.result ||
      !panes.line ||
      gradingRef.current === activePractice
    )
      return;
    gradingRef.current = activePractice;
    try {
      const result = await api.repertoireGrade(panes.line, workspaceSide, uci);
      if (activePracticeRef.current !== activePractice) return;
      setPractice({ ...activePractice, result });
      setRepertoireRefresh((n) => n + 1);
    } catch (error) {
      if (activePracticeRef.current !== activePractice) return;
      w.setDiagnostics([
        {
          code: "repertoire",
          message: (error as { message?: string }).message ?? String(error),
        },
      ]);
    } finally {
      if (gradingRef.current === activePractice) gradingRef.current = undefined;
    }
  }
  /** The best line of the active kibitzer, for the position shown. */
  function bestLine() {
    // Threat lines start after a null move, and a locked engine's lines
    // belong to its own position: neither pastes at the current move.
    const locked = k.current?.locked;
    if (
      k.current?.threat ||
      (locked &&
        (locked.documentId !== state.document?.documentId ||
          locked.nodeId !== state.position?.nodeId))
    )
      return undefined;
    const lines = k.current?.lines ?? [];
    const line = lines.find((l) => l.multipv === 1) ?? lines[0];
    return line?.rawPv?.length && game ? line : undefined;
  }
  /**
   * Paste Engine Variation (Ctrl+Space) and Paste Engine Move (Shift+Space):
   * the engine's best line, or its first move, from the current move. Moves
   * already in the notation are followed; the rest become the continuation
   * or a variation. A pasted line ends with its score and depth as text.
   */
  async function pasteEngine(whole: boolean) {
    const line = bestLine();
    if (!line?.rawPv) return;
    const comment =
      whole && line.score
        ? `${formatScore(line.score, line.rootTurn, "white")}${
            line.depth ? `/${line.depth}` : ""
          }`
        : undefined;
    const last = await e.edit({
      kind: "appendLine",
      parent: selected,
      moves: whole ? line.rawPv : line.rawPv.slice(0, 1),
      comment,
    });
    // The pasted move is played; after a pasted line the board stays.
    if (!whole && typeof last === "number") await w.selectNode(last);
  }
  /** Annotates the game's main line from the tablebases, as one edit. */
  async function runTablebaseAnnotation() {
    const doc = state.document;
    if (!doc?.documentId) return;
    const documentId = doc.documentId;
    try {
      const a = await api.tablebaseAnnotation(documentId);
      if (a.status === "noTables") {
        w.setStatus(
          "Tablebase Annotation: choose a tablebase folder in Options",
        );
        return;
      }
      if (!a.probed) {
        w.setStatus(
          "Tablebase Annotation: no position of the game is in the tables",
        );
        return;
      }
      const now = w.current.current.document;
      if (now?.documentId !== documentId || now.revision !== a.revision) {
        w.setStatus("Tablebase Annotation: the game changed; run it again");
        return;
      }
      if (a.items.length) {
        const result = await e.edit({ kind: "addAnalysis", items: a.items });
        if (result === null) return;
      }
      w.setStatus(
        `Tablebase Annotation: ${a.probed} positions in the tables, ${a.mistakes} ${a.mistakes === 1 ? "move gives" : "moves give"} away the result`,
      );
    } catch (err) {
      w.setDiagnostics([{ code: "tablebase", message: explain(err) }]);
    }
  }
  /** Runs Tactical Analysis and adds its findings as one edit. */
  async function runBlunderCheck(
    params: BlunderParams,
    storeEvaluations: boolean,
  ) {
    const doc = state.document;
    if (!doc?.documentId) return;
    const documentId = doc.documentId;
    setBlunder({ running: "starting…", documentId });
    try {
      const report = await api.blunderCheck(documentId, params, (p) =>
        setBlunder(
          (b) => b && { ...b, running: `${p.done} of ${p.total} positions` },
        ),
      );
      if (report.cancelled) {
        setBlunder({ error: "Stopped; nothing was added." });
        return;
      }
      const now = w.current.current.document;
      if (now?.documentId !== documentId || now.revision !== report.revision) {
        setBlunder({
          error: "The game changed during the analysis; run it again.",
        });
        return;
      }
      const evaluation = new Map(
        report.evaluations.map((v) => [v.node, v.nag]),
      );
      const items: Extract<Edit, { kind: "addAnalysis" }>["items"] =
        report.findings.map((f) => ({
          node: f.node,
          // "??" from three times the threshold, "?" below.
          nag: f.loss >= 3 * params.threshold ? 4 : 2,
          evaluation: evaluation.get(f.node),
          comment: storeEvaluations ? f.played : undefined,
          line: f.line,
          lineComment: `${f.best}${f.depth ? `/${f.depth}` : ""}`,
        }));
      for (const v of report.evaluations)
        if (!report.findings.some((f) => f.node === v.node))
          items.push({ node: v.node, evaluation: v.nag, line: [] });
      if (items.length) {
        const result = await e.edit({ kind: "addAnalysis", items });
        if (result === null) {
          setBlunder({ error: "The findings could not be added." });
          return;
        }
      }
      setBlunder(undefined);
      w.setStatus(
        report.findings.length
          ? `Tactical Analysis: ${report.findings.length} mistakes in ${report.checked} positions`
          : `Tactical Analysis: no mistakes in ${report.checked} positions`,
      );
    } catch (err) {
      setBlunder({ error: explain(err) });
    }
  }
  /** Runs Deep Analysis on the board position and adds its tree. */
  async function runDeepAnalysis(params: Omit<DeepParams, "node">) {
    const doc = state.document;
    if (!doc?.documentId) return;
    const documentId = doc.documentId;
    setDeep({ running: true, documentId });
    try {
      const report = await api.deepAnalysis(
        documentId,
        { ...params, node: selected },
        (p) => setDeep((d) => d?.running && { ...d, running: p }),
      );
      if (report.cancelled) {
        setDeep({ error: "Cancelled; nothing was added." });
        return;
      }
      const now = w.current.current.document;
      if (now?.documentId !== documentId || now.revision !== report.revision) {
        setDeep({
          error: "The game changed during the analysis; nothing was added.",
        });
        return;
      }
      if (report.lines.length) {
        const result = await e.edit({
          kind: "addLines",
          parent: report.node,
          lines: report.lines,
        });
        if (result === null) {
          setDeep({ error: "The analysis could not be added." });
          return;
        }
      }
      setDeep(undefined);
      w.setStatus(
        `Deep Analysis: ${report.iterations} ${report.iterations === 1 ? "iteration" : "iterations"}, ${report.positions} positions, ${report.lines.length} lines added${report.limited ? " (the tree reached its size limit)" : ""}`,
      );
    } catch (err) {
      setDeep({ error: explain(err) });
    }
  }
  /** Shows the analysis jobs; `clear` starts a new collection. */
  async function openJobs(clear: boolean) {
    try {
      const view = clear
        ? await api.clearAnalysisJobs()
        : await api.analysisJobs();
      setJobs({ view });
    } catch (err) {
      w.setDiagnostics([{ code: "jobs", message: explain(err) }]);
    }
  }
  /** Create analysis jobs: the final positions of the selected games. */
  async function createJobs() {
    const db = state.database;
    const ranges = chosenRanges();
    if (!db?.writable || !ranges.length) return;
    try {
      const r = await api.addAnalysisJobs(db.databaseId, ranges, jobKind);
      setJobs({ view: r.view });
      w.setStatus(
        `${r.view.jobs.length} analysis jobs${r.leftOut ? `; ${r.leftOut} games could not be read` : ""}`,
      );
    } catch (err) {
      w.setDiagnostics([{ code: "jobs", message: explain(err) }]);
    }
  }
  /** Add Analysis Job: the board position of the shown game. */
  async function addJob() {
    const doc = state.document;
    if (!doc?.documentId) return;
    try {
      const view = await api.addAnalysisJob(doc.documentId, selected, jobKind);
      setJobs((j) => j && { ...j, view });
      // The reference app's message 4026.
      w.setStatus("Analysis job added");
    } catch (err) {
      w.setDiagnostics([{ code: "jobs", message: explain(err) }]);
    }
  }
  async function runJobs(params: RunJobsParams) {
    setJobs(
      (j) =>
        j && {
          ...j,
          view: { ...j.view, running: true },
          error: undefined,
          progress: "Starting…",
        },
    );
    try {
      const report = await api.runAnalysisJobs(params, (p) =>
        setJobs(
          (j) =>
            j && {
              ...j,
              view: { jobs: p.jobs, running: true },
              progress: `${p.done} of ${p.total} jobs finished`,
            },
        ),
      );
      const text = `Analysis jobs ${report.stopped ? "stopped" : "finished"}: ${report.done} done, ${report.skipped} skipped, ${report.failed} failed`;
      setJobs(
        (j) =>
          j && {
            ...j,
            view: { jobs: report.jobs, running: false },
            progress: text,
          },
      );
      w.setStatus(text);
      if (report.done > 0 && state.database) {
        keepSelection.current = true;
        await w.refreshList();
      }
    } catch (err) {
      const view = await api.analysisJobs().catch(() => undefined);
      setJobs(
        (j) =>
          j && {
            view: view ?? { ...j.view, running: false },
            error: explain(err),
          },
      );
    }
  }
  async function editJob(id: number, action: "restart" | "skip" | "remove") {
    try {
      const view = await api.editAnalysisJob(id, action);
      setJobs((j) => j && { ...j, view });
    } catch (err) {
      setJobs((j) => j && { ...j, error: explain(err) });
    }
  }
  /** Draws or removes a coloured square or arrow on the current position. */
  function drawMark(mark: DrawnMark) {
    const node = game?.nodes[selected];
    if (!node || e.busy) return;
    void e.edit({
      kind: "setMarks",
      node: selected,
      ...toggleMark(node, mark),
    });
  }
  function navigate(step: Step) {
    if (!game) return;
    let id = selected;
    if (step === "first") id = 0;
    if (step === "previous") id = game.nodes[id]?.parent ?? 0;
    if (step === "next") id = game.nodes[id]?.children[0] ?? id;
    if (step === "last") {
      let count = 0;
      while (
        game.nodes[id]?.children[0] !== undefined &&
        count++ < game.nodes.length
      )
        id = game.nodes[id].children[0];
    }
    void w.selectNode(id);
  }
  // Arrow keys replay moves in both views, as in the reference app.
  useEffect(() => {
    function key(event: KeyboardEvent) {
      if (e.question || promotion.length || palette || event.defaultPrevented)
        return;
      const target = event.target as HTMLElement;
      if (
        target.closest?.(
          'input,textarea,select,[contenteditable="true"],[role="separator"]',
        )
      )
        return;
      if (event.ctrlKey || event.metaKey || event.altKey) return;
      const step = (
        {
          ArrowLeft: "previous",
          ArrowRight: "next",
          Home: "first",
          End: "last",
        } as const
      )[event.key as "ArrowLeft"];
      if (step && game) {
        event.preventDefault();
        navigate(step);
      }
    }
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });

  async function jump() {
    if (!/^[0-9]+$/.test(ordinal) || BigInt(ordinal) === 0n) {
      w.setDiagnostics([
        { code: "bounds", message: "Enter a positive decimal entry number" },
      ]);
      return;
    }
    await e.run(() => w.goToEntry(BigInt(ordinal).toString()));
  }
  // The list's multiple selection, per list.
  const listKey = `${state.database?.databaseId}:${state.listVersion}`;
  const [selection, setSelection] = useState<Selection>(noSelection);
  /** Set before a refresh that keeps every row in place (Mark Games, Edit
   * Game Data), so the selection survives it. */
  const keepSelection = useRef(false);
  useEffect(() => {
    if (keepSelection.current) keepSelection.current = false;
    else setSelection(noSelection);
  }, [listKey]);
  useEffect(() => {
    if (replay?.databaseId && replay.databaseId !== state.database?.databaseId)
      setReplay(undefined);
  }, [replay, state.database?.databaseId]);
  /** Games copied and waiting to be pasted, and a running paste. */
  const [clipGames, setClipGames] = useState(0);
  const [pastingGames, setPastingGames] = useState<string>();
  /** A maintenance job: Check Integrity, Find Double Games or Remove
   * Deleted Games, with its progress and result. */
  const [classified, setClassified] = useState<Classification>();
  const [reordering, setReordering] = useState(false);
  const [report, setReport] = useState<{
    databaseId: string;
    startFen?: string;
    running?: string;
    error?: string;
    result?: OpeningReport;
  }>();
  const [job, setJob] = useState<{
    kind: "integrity" | "doubles" | "compact" | "eco";
    databaseId: string;
    /** Find Double Games shows its options first. */
    options?: boolean;
    running?: string;
    error?: string;
    lines?: string[];
  }>();
  /** Edit Game Data for a selection: its dialog, while open. */
  const [selEdit, setSelEdit] = useState<{
    databaseId: string;
    ranges: { start: string; end: string }[];
    games: number;
    initial: {
      event?: string;
      site?: string;
      annotator?: string;
      source?: string;
    };
    running?: string;
    error?: string;
  }>();
  /** Convert to 2CBH: its dialog, while open. */
  const [converting, setConverting] = useState<{
    databaseId: string;
    name: string;
    entries: string;
    selected: number;
    ranges: { start: string; end: string }[];
    scope: ConvertScope;
    target: string;
    estimate?: ConvertEstimate;
    running?: string;
    error?: string;
  }>();
  const explain = (err: unknown) =>
    (err as { message?: string })?.message ?? String(err);
  /** The selection, or the chosen row when nothing is selected. */
  const chosenRanges = () =>
    selection.ranges.length
      ? rowRanges(selection)
      : w.cursor !== undefined
        ? rowRanges({ ranges: [[w.cursor, w.cursor + 1]] })
        : [];
  /** Convert to 2CBH: asks for the new database, then shows the estimate. */
  async function startConvert() {
    const db = state.database;
    if (!db) return;
    const target = await api.chooseConvertTarget(db.name ?? "Converted");
    if (typeof target !== "string") return;
    const selected = selectedCount(selection);
    const next = {
      databaseId: db.databaseId,
      name: db.name ?? "Database",
      entries: db.entryCount,
      selected,
      ranges: selected > 1 ? rowRanges(selection) : [],
      scope: (selected > 1 ? "selection" : "all") as ConvertScope,
      target,
    };
    setConverting(next);
    await estimateConvert(next);
  }
  async function estimateConvert(c: {
    databaseId: string;
    ranges: { start: string; end: string }[];
    scope: ConvertScope;
    target: string;
  }) {
    try {
      const estimate = await api.convertEstimate(
        c.databaseId,
        c.scope === "selection" ? c.ranges : null,
        c.target,
      );
      // An estimate for an earlier choice is dropped.
      setConverting((s) =>
        s && s.scope === c.scope && s.target === c.target
          ? { ...s, estimate, error: undefined }
          : s,
      );
    } catch (err) {
      setConverting((s) => s && { ...s, error: explain(err) });
    }
  }
  async function runConvert() {
    const c = converting;
    if (!c?.estimate?.fits || c.running) return;
    setConverting({ ...c, running: "starting…", error: undefined });
    try {
      const result = await api.convertDatabase(
        c.databaseId,
        c.scope === "selection" ? c.ranges : null,
        c.target,
        (p) =>
          setConverting(
            (s) => s && { ...s, running: `${p.done} of ${p.total} entries` },
          ),
      );
      if (result.cancelled || !result.path) {
        setConverting(
          (s) =>
            s && {
              ...s,
              running: undefined,
              error: "Stopped; nothing was written.",
            },
        );
        return;
      }
      setConverting(undefined);
      const notes = [
        result.deleted ? `${result.deleted} marked deleted` : "",
        result.incomplete
          ? `${result.incomplete} without some annotations`
          : "",
        result.texts ? `${result.texts} text entries left out` : "",
        result.chess960
          ? `${result.chess960} Chess960 games left out: 2CBH cannot hold them yet`
          : "",
        result.unreadable
          ? `${result.unreadable} unreadable entries left out`
          : "",
      ].filter(Boolean);
      await w.openWritten(result.path);
      w.setStatus(
        `${result.written} games converted${notes.length ? ` (${notes.join(", ")})` : ""}`,
      );
    } catch (err) {
      setConverting(
        (s) => s && { ...s, running: undefined, error: explain(err) },
      );
    }
  }
  const jobProgress = (p: { done: string; total: string }) =>
    setJob((j) => j && { ...j, running: `${p.done} of ${p.total} entries` });
  async function runIntegrity() {
    const db = state.database;
    if (!db) return;
    setJob({
      kind: "integrity",
      databaseId: db.databaseId,
      running: "starting…",
    });
    try {
      const r = await api.checkIntegrity(db.databaseId, jobProgress);
      const counts = Object.entries(r.counts);
      const lines = [
        `${r.cancelled ? "Stopped after" : "Checked"} ${r.checked} entries: ${r.games} games, ${r.texts} texts, ${r.deleted} marked deleted.`,
        counts.length
          ? `${counts.reduce((n, [, c]) => n + c, 0)} problems: ${counts.map(([k, c]) => `${k} ${c}`).join(", ")}.`
          : "No problems found.",
        ...r.problems
          .slice(0, 50)
          .map((p) =>
            p.ordinal
              ? `Entry ${p.ordinal}: ${p.message}`
              : `Files: ${p.message}`,
          ),
      ];
      setJob((j) => j && { ...j, running: undefined, lines });
    } catch (err) {
      setJob((j) => j && { ...j, running: undefined, error: explain(err) });
    }
  }
  /** Set ECO Codes, after its options. */
  async function runSetEco(onlyMissing: boolean) {
    const db = state.database;
    if (!db?.writable) return;
    setJob({ kind: "eco", databaseId: db.databaseId, running: "starting…" });
    try {
      const r = await api.setEcoCodes(db.databaseId, onlyMissing, jobProgress);
      const lines = r.cancelled
        ? [`Stopped; ${r.changed} codes were written before.`]
        : [
            `${r.games} games classified: ${r.changed} codes written, ${r.unchanged} already right${onlyMissing ? `, ${r.kept} other codes kept` : ""}.`,
            `${r.unclassified} games not classified (set-ups or no moves)${Number(r.unreadable) ? `; ${r.unreadable} games could not be read` : ""}.`,
            `Of the ${r.hadCode} games that had a code, ${r.agreed} already had the table's.`,
          ];
      setJob((j) => j && { ...j, running: undefined, lines });
      if (Number(r.changed) > 0) {
        keepSelection.current = true;
        await w.refreshList();
      }
    } catch (err) {
      setJob((j) => j && { ...j, running: undefined, error: explain(err) });
    }
  }
  /** Opening Report of the board position, from the open database. */
  async function openingReport() {
    const db = state.database;
    const line = game && lineTo(game, selected);
    if (!db || !line) return;
    setReport({
      databaseId: db.databaseId,
      startFen: line.startFen,
      running: "Starting…",
    });
    try {
      const result = await api.openingReport(db.databaseId, line, (p) =>
        setReport(
          (r) =>
            r && {
              ...r,
              running:
                p.total === "1"
                  ? p.stage
                  : `${p.stage}: ${p.done} of ${p.total}`,
            },
        ),
      );
      setReport(
        (r) =>
          r && {
            ...r,
            running: undefined,
            result,
            error: result.cancelled ? "Stopped." : undefined,
          },
      );
    } catch (err) {
      setReport((r) => r && { ...r, running: undefined, error: explain(err) });
    }
  }
  async function classifyOpening() {
    const doc = state.document;
    if (!doc?.documentId) return;
    try {
      setClassified(await api.classifyGame(doc.documentId));
    } catch (err) {
      w.setDiagnostics([{ code: "classify", message: explain(err) }]);
    }
  }
  async function runDoubles(options: DoubleOptions) {
    const db = state.database;
    if (!db) return;
    setJob({
      kind: "doubles",
      databaseId: db.databaseId,
      options: true,
      running: "starting…",
    });
    try {
      const r = await api.findDoubleGames(db.databaseId, options, jobProgress);
      await w.refreshList();
      const lines = r.cancelled
        ? ["Stopped; nothing was marked."]
        : [
            `Compared ${r.checked} entries: ${r.marked} doubles marked deleted.`,
            ...(r.keptBoth !== "0"
              ? [
                  `${r.keptBoth} pairs kept both games (annotated, or different annotators).`,
                ]
              : []),
            ...(r.treeStale ? ["The opening tree needs rebuilding."] : []),
            ...r.pairs
              .slice(0, 50)
              .map(([gone, kept]) => `Game ${gone} marked; game ${kept} kept.`),
          ];
      setJob((j) => j && { ...j, options: false, running: undefined, lines });
    } catch (err) {
      setJob((j) => j && { ...j, running: undefined, error: explain(err) });
    }
  }
  async function runCompact() {
    const db = state.database;
    if (!db?.writable || !(await api.confirmRemoveDeleted())) return;
    setJob({
      kind: "compact",
      databaseId: db.databaseId,
      running: "starting…",
    });
    try {
      const r = await api.removeDeletedGames(db.databaseId, jobProgress);
      if (r.cancelled) {
        setJob(
          (j) =>
            j && {
              ...j,
              running: undefined,
              lines: ["Stopped; the database is unchanged."],
            },
        );
        return;
      }
      w.renumber(db.databaseId, r.moved, r.entryCount);
      const mb = (b: string) => `${(Number(b) / 1e6).toFixed(1)} MB`;
      setJob(
        (j) =>
          j && {
            ...j,
            running: undefined,
            lines: [
              `${r.removed} deleted games removed; ${r.kept} entries kept.`,
              `The files took ${mb(r.bytesBefore)} and now take ${mb(r.bytesAfter)}.`,
              ...(r.moved.length
                ? [
                    `${r.moved.length} open games follow their new entry numbers.`,
                  ]
                : []),
              ...(r.treeDeleted
                ? ["The opening tree was deleted; create it again when needed."]
                : []),
            ],
          },
      );
    } catch (err) {
      setJob((j) => j && { ...j, running: undefined, error: explain(err) });
    }
  }
  /** Mark Games (+): toggles the user medal of the selection's games. */
  async function markGames() {
    const db = state.database;
    const ranges = chosenRanges();
    if (!db?.writable || !ranges.length) return;
    try {
      const r = await api.markRows(db.databaseId, ranges, () => {});
      keepSelection.current = true;
      await w.refreshList();
      // The reference app's message 50885.
      w.setStatus(`${r.changed} Games have been marked or unmarked`);
    } catch (err) {
      w.setDiagnostics([{ code: "mark", message: explain(err) }]);
    }
  }
  function startSelectionEdit() {
    const db = state.database;
    const ranges = chosenRanges();
    if (!db?.writable || !ranges.length) return;
    const first = Number(ranges[0].start) - 1;
    const row = w.rows.row(first);
    setSelEdit({
      databaseId: db.databaseId,
      ranges,
      games: ranges.reduce((n, r) => n + Number(r.end) - Number(r.start), 0),
      initial: {
        event: row?.event,
        site: row?.site,
        annotator: row?.annotator,
        source: row?.source,
      },
    });
  }
  async function runSelectionEdit(names: SelectionNames) {
    const s = selEdit;
    if (!s || s.running) return;
    setSelEdit({ ...s, running: "starting…", error: undefined });
    try {
      const r = await api.setNamesOfRows(s.databaseId, s.ranges, names, (p) =>
        setSelEdit(
          (x) => x && { ...x, running: `${p.done} of ${p.total} games` },
        ),
      );
      setSelEdit(undefined);
      // A list sorted by a name column may reorder after new names.
      keepSelection.current = !w.current.current.sort;
      await w.refreshList();
      w.setStatus(
        `Game data set for ${r.changed} games${r.cancelled ? " (stopped)" : ""}`,
      );
    } catch (err) {
      setSelEdit((x) => x && { ...x, running: undefined, error: explain(err) });
    }
  }
  async function copyGames() {
    const db = state.database;
    const ranges = chosenRanges();
    if (!db || !ranges.length) return;
    try {
      const copied = await api.copyRows(db.databaseId, ranges);
      setClipGames(Number(copied.games));
      w.setStatus(
        `${copied.games} games copied${copied.truncated ? " (stopped at the limit)" : ""}`,
      );
    } catch (err) {
      w.setDiagnostics([{ code: "copy", message: explain(err) }]);
    }
  }
  async function pasteGames() {
    const db = state.database;
    if (!db?.writable || !clipGames) return;
    if (!(await api.confirmPaste(String(clipGames)))) return;
    setPastingGames("starting…");
    try {
      const result = await api.pasteGames(db.databaseId, (p) =>
        setPastingGames(`${p.done} of ${p.total}`),
      );
      w.setState((s) => ({
        ...s,
        database:
          s.database?.databaseId === db.databaseId
            ? { ...s.database, entryCount: result.entryCount }
            : s.database,
      }));
      await w.refreshList();
      const notes = [
        result.cancelled ? "cancelled" : "",
        result.skipped !== "0" ? `${result.skipped} skipped` : "",
        result.incomplete !== "0"
          ? `${result.incomplete} without some annotations`
          : "",
        result.chess960 && result.chess960 !== "0"
          ? `${result.chess960} Chess960 games left out: 2CBH cannot hold them yet`
          : "",
        result.treeStale ? "the opening tree needs rebuilding" : "",
      ].filter(Boolean);
      w.setStatus(
        `${result.pasted} games pasted${notes.length ? ` (${notes.join(", ")})` : ""}`,
      );
    } catch (err) {
      w.setDiagnostics([{ code: "paste", message: explain(err) }]);
    } finally {
      setPastingGames(undefined);
    }
  }
  /** Delete: the selection's games, or the loaded game. */
  async function deleteGames() {
    const db = state.database;
    if (!db?.writable) return;
    if (selectedCount(selection) <= 1)
      return e.toggleDeleted(state.document?.summary.ordinal ?? "");
    try {
      const changed = await api.deleteRows(db.databaseId, rowRanges(selection));
      await w.refreshList();
      w.setStatus(`${changed} games changed`);
    } catch (err) {
      w.setDiagnostics([{ code: "delete", message: explain(err) }]);
    }
  }
  // Games from a position search open at the position found.
  const preview = (
    row: number,
    mods?: { toggle: boolean; extend: boolean },
  ) => {
    setSelection((s) => chooseRow(s, row, mods));
    // Ctrl and Shift only change the selection.
    if (mods?.toggle || mods?.extend) return;
    void (async () => {
      const entry = await w.rows.entry(row, w.rowCount);
      // A second click on the game shown, e.g. a double-click, loads nothing.
      if (!entry || (await w.showing(entry.ordinal))) return;
      await e.run(() => w.previewRow(row, entry));
    })();
  };
  async function open(row: number) {
    const entry = await w.rows.entry(row, w.rowCount);
    if (!entry) return;
    if (!(await w.showing(entry.ordinal)))
      await e.run(() => w.previewRow(row, entry));
    if (await w.showing(entry.ordinal)) await e.openPreview();
  }
  const step = (delta: 1 | -1) => void e.run(() => w.stepGame(delta));
  function showList(tab: "games" | NameKind) {
    setView("database");
    setListTab(tab);
    if (tab !== "games")
      setOpenedTabs((t) => (t.includes(tab) ? t : [...t, tab]));
  }
  /** Shows the games of a player or tournament in the game list. */
  function gamesOf(kind: NameKind, row: NameRow) {
    const name = row.name ?? "?";
    showList("games");
    return w.search(
      {
        ...emptyFilter(),
        ...(kind === "players"
          ? { playerId: row.id }
          : { tournamentId: row.id }),
      },
      kind === "players"
        ? `Games of ${name}`
        : `${name}${row.site ? `, ${row.site}` : ""}`,
    );
  }
  async function openCrosstable(tournamentId: number, layout?: CrossLayout) {
    const db = state.database;
    if (!db) return;
    setCross({ tournamentId, loading: true });
    try {
      const table = await api.crosstable(
        db.databaseId,
        tournamentId,
        layout ?? null,
      );
      setCross((c) =>
        c?.tournamentId === tournamentId ? { tournamentId, table } : c,
      );
    } catch (e) {
      setCross((c) =>
        c?.tournamentId === tournamentId
          ? {
              tournamentId,
              error: (e as { message?: string })?.message ?? String(e),
            }
          : c,
      );
    }
  }
  /** Error Report: the selected games and their players. */
  async function openErrorReport() {
    const db = state.database;
    const ranges = chosenRanges();
    if (!db || !ranges.length) return;
    const selected = ranges.reduce(
      (n, r) => n + Number(r.end) - Number(r.start),
      0,
    );
    try {
      const players = await api.selectionPlayers(db.databaseId, ranges);
      setErrors({ databaseId: db.databaseId, ranges, selected, players });
    } catch (err) {
      w.setDiagnostics([{ code: "errorReport", message: explain(err) }]);
    }
  }
  /** Shows the round-by-round report of a tournament of the database. */
  async function openReport(tournamentId: number) {
    const db = state.database;
    if (!db) return;
    setTourReport({ tournamentId, loading: true });
    try {
      const data = await api.tournamentReport(db.databaseId, tournamentId);
      setTourReport((c) =>
        c?.tournamentId === tournamentId ? { tournamentId, data } : c,
      );
    } catch (e) {
      setTourReport((c) =>
        c?.tournamentId === tournamentId
          ? {
              tournamentId,
              error: (e as { message?: string })?.message ?? String(e),
            }
          : c,
      );
    }
  }
  /** Shows the statistics of a player of the active database. */
  async function openStatistics(
    playerId: number,
    players?: { id: number; name?: string }[],
  ) {
    const db = state.database;
    if (!db) return;
    const databaseId = db.databaseId;
    const live = (c?: { databaseId: string; playerId: number }) =>
      c?.databaseId === databaseId && c.playerId === playerId;
    setStats((c) => ({
      databaseId,
      playerId,
      progress: "Reading the games…",
      players: players ?? c?.players,
    }));
    try {
      const data = await api.playerStatistics(databaseId, playerId, (p) => {
        const pct = Number((BigInt(p.done) * 100n) / BigInt(p.total || "1"));
        setStats((c) =>
          live(c) ? { ...c!, progress: `Reading the games, ${pct}%` } : c,
        );
      });
      setStats((c) =>
        live(c)
          ? data
            ? { ...c!, data, progress: undefined }
            : { ...c!, progress: undefined, error: "Statistics cancelled" }
          : c,
      );
    } catch (e) {
      setStats((c) =>
        live(c)
          ? {
              ...c!,
              progress: undefined,
              error: (e as { message?: string })?.message ?? String(e),
            }
          : c,
      );
    }
  }
  /** Prepares against a player of the active database with one colour. */
  async function openPreparation(
    playerId: number,
    white: boolean,
    name?: string,
  ) {
    const db = state.database;
    if (!db) return;
    const databaseId = db.databaseId;
    const live = (c?: {
      databaseId: string;
      playerId: number;
      white: boolean;
    }) =>
      c?.databaseId === databaseId &&
      c.playerId === playerId &&
      c.white === white;
    setPrep((c) => ({
      databaseId,
      playerId,
      white,
      name: name ?? (c?.playerId === playerId ? c.name : undefined),
      progress: "Finding the games…",
    }));
    try {
      const data = await api.prepareAgainst(
        databaseId,
        playerId,
        white,
        (p) => {
          const pct = Number((BigInt(p.done) * 100n) / BigInt(p.total || "1"));
          const what =
            p.stage === "games" ? "Finding the games" : "Reading the moves";
          setPrep((c) =>
            live(c) ? { ...c!, progress: `${what}, ${pct}%` } : c,
          );
        },
      );
      // The backend keeps one preparation per database: this one.
      setBookPrep((b) =>
        b?.databaseId === databaseId &&
        (b.playerId !== playerId || b.white !== white)
          ? undefined
          : b,
      );
      setPrep((c) =>
        live(c)
          ? data
            ? { ...c!, data, name: data.name, progress: undefined }
            : { ...c!, progress: undefined, error: "Preparation stopped" }
          : c,
      );
    } catch (e) {
      setPrep((c) =>
        live(c)
          ? {
              ...c!,
              progress: undefined,
              error: (e as { message?: string })?.message ?? String(e),
            }
          : c,
      );
    }
  }
  /** Builds the dossier of a player of the active database. */
  async function openDossier(playerId: number, name?: string) {
    const db = state.database;
    if (!db) return;
    const databaseId = db.databaseId;
    const live = (c?: { databaseId: string; playerId: number }) =>
      c?.databaseId === databaseId && c.playerId === playerId;
    setDossier({ databaseId, playerId, name, progress: "Finding the games…" });
    const stages = {
      games: "Finding the games",
      lines: "Building the repertoire",
      mates: "Looking for mates",
    };
    try {
      const data = await api.playerDossier(databaseId, playerId, (p) => {
        const pct = Number((BigInt(p.done) * 100n) / BigInt(p.total || "1"));
        setDossier((c) =>
          live(c) ? { ...c!, progress: `${stages[p.stage]}, ${pct}%` } : c,
        );
      });
      setDossier((c) =>
        live(c)
          ? data
            ? { ...c!, data, progress: undefined }
            : { ...c!, progress: undefined, error: "Dossier stopped" }
          : c,
      );
    } catch (e) {
      setDossier((c) =>
        live(c)
          ? {
              ...c!,
              progress: undefined,
              error: (e as { message?: string })?.message ?? String(e),
            }
          : c,
      );
    }
  }
  /** The players of the shown game, when it comes from the active database. */
  const gamePlayers: { id: number; name?: string }[] = [];
  if (
    state.document &&
    state.database &&
    state.origin?.databaseId === state.database.databaseId
  ) {
    const { whiteId, blackId, white, black } = state.document.summary;
    if (whiteId !== undefined) gamePlayers.push({ id: whiteId, name: white });
    if (blackId !== undefined) gamePlayers.push({ id: blackId, name: black });
  }
  // A database opened from anywhere closes the start screen; its lists
  // changed.
  const openedId = state.database?.databaseId;
  useEffect(() => {
    if (!openedId) return;
    setStartOpen(false);
    setStartRefresh((n) => n + 1);
  }, [openedId]);
  /** The start screen takes the database view's place. */
  const startShown = view === "database" && (startOpen || !state.database);
  /** The player for Player statistic: the chosen one, else the game's. */
  const statsPlayer =
    listTab === "players" && chosenName.players
      ? chosenName.players.id
      : gamePlayers[0]?.id;
  /** The player to prepare against with a colour: the chosen one, else the
   * shown game's player of that colour. */
  const prepPlayer = (white: boolean) =>
    listTab === "players" && chosenName.players
      ? chosenName.players
      : state.document && state.database
        ? gamePlayers.find(
            (p) =>
              p.id ===
              (white
                ? state.document!.summary.whiteId
                : state.document!.summary.blackId),
          )
        : undefined;
  /** The tournament for Cross Table: the chosen one, else the game's. */
  const crossTournament =
    listTab === "tournaments" && chosenName.tournaments
      ? chosenName.tournaments.id
      : state.document?.summary.tournamentId;
  /** Rows the list shows at once, for Page Up and Page Down. */
  const [visibleRows, setVisibleRows] = useState(30);
  function scrollPage(delta: 1 | -1) {
    w.setListTop((top) =>
      Math.max(
        0,
        Math.min(
          top + delta * visibleRows,
          Math.max(0, w.rowCount - visibleRows),
        ),
      ),
    );
  }

  /** Plays a book move: the existing continuation, or a new move. */
  function playBookMove(uci: string) {
    if (!game || e.busy) return;
    const existing = game.nodes[selected]?.children.find(
      (c) => game.nodes[c]?.uci === uci,
    );
    if (existing !== undefined) void w.selectNode(existing);
    else void e.edit({ kind: "appendMove", parent: selected, uci });
  }
  // Games from the reference list open in a tab of their own.
  const openReference = (ordinal: string, node: number) =>
    void e.openEntry(ordinal, node);
  const bookPane = (
    <BookPane
      book={panes.book}
      hasDatabase={!!state.database}
      hasLine={!!panes.line}
      onPlay={playBookMove}
      onCreate={() => void w.createTree()}
      building={w.treeBuild && `Building the opening tree: ${w.treeBuild}`}
      searchAvailable={!!state.search && !bookPrepared}
      searchTruncated={!!state.search?.truncated}
      scope={
        bookScope === "search" && state.search && !bookPrepared
          ? "search"
          : "database"
      }
      onScopeChange={setBookScope}
      prepared={bookPrepared?.label}
      onLeavePrepared={() => {
        if (!bookPrepared) return;
        setBookPrep(undefined);
        void api.closePreparation(bookPrepared.databaseId).catch(() => {});
      }}
    />
  );
  const referencePane = (
    <ReferencePane
      reference={panes.reference}
      api={api}
      databaseId={state.database?.databaseId}
      filter={panes.referenceFilter}
      onFilterChange={panes.setReferenceFilter}
      hasDatabase={!!state.database}
      hasLine={!!panes.line}
      onOpen={openReference}
      onSearch={panes.searchReference}
      onCancel={panes.cancelReference}
    />
  );
  const repertoirePane = (
    <RepertoirePane
      api={api}
      line={panes.line}
      side={workspaceSide}
      onSideChange={(side) => {
        setWorkspaceSide(side);
        setPractice(undefined);
      }}
      book={panes.book}
      onNavigate={(fen) => {
        setPractice(undefined);
        void e.newGame(fen);
      }}
      onPractice={() => {
        if (panes.line)
          setPractice({ lineKey: currentLineKey, side: workspaceSide });
      }}
      onStopPractice={() => setPractice(undefined)}
      practising={!!activePractice}
      practice={activePractice?.result}
      refresh={repertoireRefresh}
    />
  );

  /** Shows the notation's text editor with the given field focused. */
  function focusText(field: "after" | "before") {
    setNotationTab("notation");
    setFocusField(field);
    setCommentFocus((n) => n + 1);
  }
  /** A move is in a variation when it, or a move before it, is not the
   * first continuation of its parent. */
  const inVariation = useMemo(() => {
    if (!game) return false;
    let id = selected;
    for (let steps = 0; id !== 0 && steps <= game.nodes.length; steps++) {
      const parent = game.nodes[id]?.parent;
      if (parent === undefined) return false;
      if (game.nodes[parent].children[0] !== id) return true;
      id = parent;
    }
    return false;
  }, [game, selected]);
  /** Pastes a game (PGN) or position (FEN) from the clipboard, or asks for
   * the text when the clipboard cannot be read. */
  async function paste(kind: "game" | "position") {
    let text = "";
    try {
      text = (await api.readClipboard()) ?? "";
    } catch {
      // Reading may be refused; the dialog takes the text instead.
    }
    if (!text.trim()) {
      setPasting(kind);
      return;
    }
    await pasteText(kind, text);
  }
  const pasteText = (kind: "game" | "position", text: string) =>
    e.run(async () => {
      if (kind === "game") await e.pasteGame(text);
      else await e.newGame(text.trim());
    }, false);

  const busy = e.busy;
  const bindings: Bindings = {
    // Opening a database keeps the game; the database gets its own tab.
    "open-database": { run: () => w.openDatabase() },
    "open-database-edit": { run: () => w.openDatabaseForEditing() },
    "new-database": { run: () => w.createDatabase() },
    "save-game": {
      // Without a database open for editing, Save exports PGN.
      run: () =>
        state.database?.writable ? e.saveToDatabase("save") : e.save(),
      enabled: !!game && !busy,
    },
    "replace-game": {
      run: () => e.saveToDatabase("replace"),
      enabled:
        !!game &&
        !busy &&
        !!state.database?.writable &&
        state.origin?.databaseId === state.database.databaseId,
    },
    "db-copy": {
      run: () => copyGames(),
      enabled: !!state.database && w.rowCount > 0,
    },
    "db-paste": {
      run: () => pasteGames(),
      enabled: !!state.database?.writable && clipGames > 0 && !pastingGames,
    },
    "mark-games": {
      run: () => markGames(),
      enabled: !!state.database?.writable && w.rowCount > 0,
    },
    "db-edit-game-data": {
      run: () => startSelectionEdit(),
      enabled: !!state.database?.writable && w.rowCount > 0 && !selEdit,
    },
    "check-integrity": {
      run: () => runIntegrity(),
      enabled: !!state.database && !job,
    },
    "find-double-games": {
      run: () =>
        state.database &&
        setJob({
          kind: "doubles",
          databaseId: state.database.databaseId,
          options: true,
        }),
      enabled: !!state.database?.writable && !job,
    },
    "remove-deleted-games": {
      run: () => runCompact(),
      enabled: !!state.database?.writable && !job,
    },
    "convert-to-2cbh": {
      run: () => startConvert(),
      enabled: !!state.database && !converting,
    },
    "db-select-all": {
      run: () => setSelection(selectAll(w.rowCount)),
      enabled: w.rowCount > 0,
    },
    "delete-game": {
      run: () => deleteGames(),
      enabled:
        !!state.database?.writable &&
        (selectedCount(selection) > 1 ||
          state.origin?.databaseId === state.database.databaseId),
    },
    "open-pgn": { run: () => e.run(e.openPgn, false) },
    "open-board": { run: () => setView("game"), enabled: !!game },
    "database-window": { run: () => setView("database") },
    // The reference app's Search text (GENERALSEARCH); what its own command opens
    // was not traced, so the workbench opens the Annotations tab.
    "search-text": {
      run: () => setMaskOpen("annotations"),
      enabled: !!state.database,
    },
    "filter-games": {
      run: () => setMaskOpen(true),
      enabled: !!state.database,
      pressed: !!state.search,
    },
    "quick-filter": {
      run: () => setFilterOpen((o) => !o),
      enabled: !!state.database,
      pressed: filterOpen,
    },
    "find-position-db": {
      run: () => {
        const fen = state.position?.fen;
        if (!fen) return;
        setView("database");
        return w.findPosition(fen);
      },
      enabled: !!state.position && !!state.database,
    },
    "open-book": {
      run: () => {
        setView("game");
        setNotationTab("book");
      },
      enabled: !!game,
      pressed: inGame && notationTab === "book",
    },
    "toggle-book-pane": {
      run: () => setExtraBook((v) => !v),
      pressed: extraBook,
    },
    "toggle-reference-pane": {
      run: () => setExtraReference((v) => !v),
      pressed: extraReference,
    },
    "find-reference": {
      run: () => {
        setView("game");
        setNotationTab("reference");
      },
      enabled: !!state.position && !!state.database,
    },
    "create-opening-tree": {
      run: () => w.createTree(),
      enabled: !!state.database && !w.treeBuild,
    },
    "delete-opening-tree": {
      run: () => w.deleteTree(),
      enabled: !!state.database?.tree && !w.treeBuild,
    },
    "create-search-booster": {
      run: () => w.createBooster(),
      enabled: !!state.database,
    },
    "delete-search-booster": {
      run: () => w.deleteBooster(),
      enabled: !!state.database?.booster,
    },
    "reset-filter": {
      run: () => w.clearSearch(),
      enabled: !!state.search,
    },
    "toggle-game-list": {
      run: () => showList("games"),
      pressed: view === "database" && listTab === "games",
    },
    "toggle-players": {
      run: () => showList("players"),
      enabled: !!state.database,
      pressed: view === "database" && listTab === "players",
    },
    "toggle-tournaments": {
      run: () => showList("tournaments"),
      enabled: !!state.database,
      pressed: view === "database" && listTab === "tournaments",
    },
    "games-of-player": {
      run: () => {
        const kind = listTab === "games" ? undefined : listTab;
        const row = kind && chosenName[kind];
        if (kind && row) return gamesOf(kind, row);
      },
      enabled: listTab !== "games" && !!chosenName[listTab],
    },
    "my-databases": {
      run: () => {
        setView("database");
        setStartOpen((o) => !o || !state.database);
      },
      pressed: view === "database" && (startOpen || !state.database),
    },
    "toggle-folder-tree": {
      run: () => setShowFolders((f) => !f),
      pressed: showFolders,
      enabled: startShown,
    },
    "add-folder-shortcut": {
      run: () =>
        void api.chooseFolder().then((path) => {
          if (typeof path !== "string") return;
          void api
            .addFolderShortcut(path)
            .then(() => setStartRefresh((n) => n + 1))
            .catch((err: unknown) =>
              w.setDiagnostics([{ code: "folder", message: explain(err) }]),
            );
        }),
    },
    "remove-database-symbol": {
      run: () => {
        const s = startSymbol;
        if (s?.favorite)
          void api
            .unpinDatabase(s.path)
            .then(() => setStartRefresh((n) => n + 1));
      },
      enabled: startShown && !!startSymbol?.favorite,
    },
    "view-symbols": {
      run: () => changeStartView("symbols"),
      pressed: startView === "symbols",
      enabled: startShown,
    },
    "view-details": {
      run: () => changeStartView("details"),
      pressed: startView === "details",
      enabled: startShown,
    },
    "view-tile": {
      run: () => changeStartView("tile"),
      pressed: startView === "tile",
      enabled: startShown,
    },
    "view-small-symbols": {
      run: () => changeStartView("smallSymbols"),
      pressed: startView === "smallSymbols",
      enabled: startShown,
    },
    "database-properties": {
      run: () => setPropertySymbol(startSymbol),
      enabled: startShown && !!startSymbol,
    },
    "recover-database": {
      run: () =>
        setRecovery({ source: startShown ? startSymbol?.path : undefined }),
      enabled: !recovery,
    },
    statistics: {
      run: () => {
        const row = chosenName.players;
        if (listTab === "players" && row) return openStatistics(row.id);
      },
      enabled: listTab === "players" && !!chosenName.players,
    },
    // The player chosen in the Players list, else the shown game's player
    // of that colour (the workbench's addition).
    "prepare-white": {
      run: () => {
        const p = prepPlayer(true);
        if (p) return openPreparation(p.id, true, p.name);
      },
      enabled: !!prepPlayer(true),
    },
    "prepare-black": {
      run: () => {
        const p = prepPlayer(false);
        if (p) return openPreparation(p.id, false, p.name);
      },
      enabled: !!prepPlayer(false),
    },
    // The reference app opens the Players list, where the player is chosen and
    // prepared against; with a player chosen, the workbench starts at once.
    "prepare-against": {
      run: () => {
        const row = chosenName.players;
        if (listTab === "players" && row)
          return openPreparation(row.id, true, row.name);
        showList("players");
      },
      enabled: !!state.database,
    },
    "style-report": {
      run: () => {
        if (!state.database || statsPlayer === undefined) return;
        const name =
          listTab === "players" && chosenName.players
            ? chosenName.players.name
            : gamePlayers.find((p) => p.id === statsPlayer)?.name;
        setPlayingStyle({
          databaseId: state.database.databaseId,
          databaseName: state.database.name ?? "Database",
          player: { id: statsPlayer, name },
        });
      },
      enabled: !!state.database && statsPlayer !== undefined,
    },
    "player-dossier": {
      run: () => {
        if (statsPlayer === undefined) return;
        const fromList = listTab === "players" && !!chosenName.players;
        const name = fromList
          ? chosenName.players?.name
          : gamePlayers.find((p) => p.id === statsPlayer)?.name;
        return openDossier(statsPlayer, name);
      },
      enabled: !!state.database && statsPlayer !== undefined,
    },
    "player-statistic": {
      run: () => {
        if (statsPlayer === undefined) return;
        const fromList = listTab === "players" && !!chosenName.players;
        return openStatistics(statsPlayer, fromList ? undefined : gamePlayers);
      },
      enabled: !!state.database && statsPlayer !== undefined,
    },
    "cross-table": {
      run: () =>
        crossTournament !== undefined && openCrosstable(crossTournament),
      enabled: !!state.database && crossTournament !== undefined,
    },
    "generate-repertoire": {
      run: () =>
        state.database &&
        setRepertoire({ databaseId: state.database.databaseId }),
      enabled: !!state.database,
    },
    "repertoire-report-white": {
      run: () =>
        state.database &&
        setRepertoire({
          databaseId: state.database.databaseId,
          report: { white: true },
        }),
      enabled: !!state.database,
    },
    "repertoire-report-black": {
      run: () =>
        state.database &&
        setRepertoire({
          databaseId: state.database.databaseId,
          report: { white: false },
        }),
      enabled: !!state.database,
    },
    "error-report": {
      run: () => openErrorReport(),
      enabled: !!state.database && w.rowCount > 0,
    },
    "tournament-report": {
      run: () => {
        const id =
          view === "game"
            ? state.document?.summary.tournamentId
            : crossTournament;
        if (id !== undefined) return openReport(id);
      },
      enabled:
        !!state.database &&
        (view === "game"
          ? state.document?.summary.tournamentId !== undefined &&
            state.origin?.databaseId === state.database.databaseId
          : crossTournament !== undefined),
    },
    "show-crosstable": {
      run: () => {
        const id = state.document?.summary.tournamentId;
        if (id !== undefined) return openCrosstable(id);
      },
      enabled:
        !!state.database &&
        state.document?.summary.tournamentId !== undefined &&
        state.origin?.databaseId === state.database.databaseId,
    },
    "previous-page": {
      run: () => scrollPage(-1),
      enabled: !!state.database && w.listTop > 0,
    },
    "next-page": {
      run: () => scrollPage(1),
      enabled: !!state.database && w.listTop + visibleRows < w.rowCount,
    },
    "save-pgn": { run: () => e.save(), enabled: !!game && !busy },
    "close-game": {
      run: () => e.run(e.closeDocument),
      enabled: !!game && !busy,
    },
    "copy-position": {
      run: () => navigator.clipboard?.writeText(state.position?.fen ?? ""),
      enabled: !!state.position,
    },
    "flip-board": { run: () => setFlipped((f) => !f), pressed: flipped },
    "add-kibitzer": {
      run: () => (settings.engines.length ? setEnginesOpen(true) : k.choose()),
      enabled: !k.adding,
    },
    "remove-kibitzer": {
      run: () => k.remove(),
      enabled: !!k.current && !k.current.busy,
    },
    "remove-all-kibitzers": {
      run: () => k.removeAll(),
      enabled: k.kibitzers.length > 0,
    },
    "default-kibitzer": {
      run: async () => {
        if (!(await k.toggleDefault())) setEnginesOpen(true);
      },
      enabled: !k.adding,
      pressed: k.kibitzers.some(
        (v) => v.installed && v.installed.id === settings.defaultEngine,
      ),
    },
    "engine-management": { run: () => setEnginesOpen(true) },
    "opening-report": {
      run: () => openingReport(),
      enabled: !!state.database && !!game && !report?.running,
    },
    "classify-opening": {
      run: () => void classifyOpening(),
      enabled: !!state.document?.documentId,
    },
    "set-eco-codes": {
      run: () =>
        state.database &&
        setJob({
          kind: "eco",
          databaseId: state.database.databaseId,
          options: true,
        }),
      enabled: !!state.database?.writable && !job,
    },
    "deep-analysis": {
      run: () => setDeep((d) => d ?? {}),
      enabled: !!state.document?.documentId,
      pressed: !!deep?.running,
    },
    "new-analysis-jobs": {
      run: () => void openJobs(true),
      enabled: !jobs?.view.running,
    },
    "open-analysis-jobs": { run: () => void openJobs(false) },
    "add-analysis-job": {
      run: () => void addJob(),
      enabled: !!state.document?.documentId,
    },
    "create-analysis-jobs": {
      run: () => void createJobs(),
      enabled: !!state.database?.writable && w.rowCount > 0,
    },
    "centipawn-list": {
      run: () => {
        const db = state.database;
        const ranges = chosenRanges();
        if (!db?.writable || !ranges.length) return;
        setCentipawnList({
          databaseId: db.databaseId,
          name: db.name ?? "Database",
          ranges,
          selected: ranges.reduce(
            (n, r) => n + Number(r.end) - Number(r.start),
            0,
          ),
        });
      },
      enabled: !!state.database?.writable && w.rowCount > 0,
    },
    "training-annotation": {
      run: () => selected !== 0 && setTraining({ node: selected }),
      enabled: !!state.document?.documentId && selected !== 0,
    },
    "replay-training-game": {
      run: () => {
        if (game && state.document?.documentId)
          setReplay({
            source: {
              kind: "current",
              game,
              documentId: state.document.documentId,
              label: `${game.tags.White ?? "White"}–${game.tags.Black ?? "Black"}`,
            },
            databaseId: state.database?.databaseId,
          });
      },
      enabled: !!game && !!state.document?.documentId,
    },
    "replay-training-selection": {
      run: () => {
        if (!state.database) return;
        const ranges = selection.ranges.length
          ? selection.ranges.map(([a, b]) => [a, b] as [number, number])
          : w.cursor !== undefined
            ? [[w.cursor, w.cursor + 1] as [number, number]]
            : [];
        if (ranges.length)
          setReplay({
            source: {
              kind: "selection",
              databaseId: state.database.databaseId,
              ranges,
              entry: (row) => w.rows.entry(row, w.rowCount),
            },
            databaseId: state.database.databaseId,
          });
      },
      enabled:
        !!state.database &&
        (selectedCount(selection) > 0 || w.cursor !== undefined),
    },
    "delete-training": {
      run: () => void e.edit({ kind: "clearTraining" }),
      enabled: !!game?.nodes.some((n) => n.training),
    },
    "centipawn-analysis": {
      run: () =>
        state.document?.documentId && setCentipawn(state.document.documentId),
      enabled: !!state.document?.documentId,
    },
    "tactical-analysis": {
      run: () => setBlunder((b) => b ?? {}),
      enabled: !!state.document?.documentId,
      pressed: !!blunder?.running,
    },
    "toggle-analysis": {
      run: () => (k.current?.enabled ? k.stop() : k.start()),
      enabled: !!k.current && !!state.document?.documentId && !k.current.busy,
      pressed: !!k.current?.enabled,
      label: k.current?.enabled ? "Stop Analysis" : "Infinite Analysis",
    },
    "increase-lines": {
      run: () => k.lines(1),
      enabled: !!k.current && !k.current.busy,
    },
    "decrease-lines": {
      run: () => k.lines(-1),
      enabled: !!k.current && !k.current.busy && k.current.multipv > 1,
    },
    "toggle-ev-profile": {
      run: () =>
        saveSettings({
          ...settings,
          evalProfilePane: !settings.evalProfilePane,
        }),
      enabled: !!game,
      pressed: !!settings.evalProfilePane,
    },
    "nota-eval-profile": {
      run: () =>
        saveSettings({
          ...settings,
          evalProfileNotation: !settings.evalProfileNotation,
        }),
      enabled: !!game,
      pressed: !!settings.evalProfileNotation,
    },
    "instant-analysis": {
      run: () => {
        const now = settings.instantAnalysis ?? defaultInstant;
        return saveSettings({
          ...settings,
          instantAnalysis: { ...now, enabled: !now.enabled },
          // The curve is where its results show.
          evalProfilePane: !now.enabled || settings.evalProfilePane,
        });
      },
      pressed: !!settings.instantAnalysis?.enabled,
    },
    "find-novelty": {
      run: async () => {
        const doc = state.document?.documentId;
        const db = state.database?.databaseId;
        if (!doc || !db) return;
        setNovelty({ documentId: doc, running: true });
        try {
          const report = await api.findNovelty(doc, db);
          setNovelty({ documentId: doc, running: false, report });
          if (report.novelty && w.current.current.document?.documentId === doc)
            void w.selectNode(report.novelty.node);
        } catch (err) {
          setNovelty({ documentId: doc, running: false, error: explain(err) });
        }
      },
      enabled:
        !!state.document?.documentId && !!state.database && !novelty?.running,
    },
    "set-medal": {
      run: () => setExtraDialog({ kind: "medal", node: selected }),
      enabled: !!state.document?.documentId && !e.busy,
    },
    "move-time": {
      run: () => setExtraDialog({ kind: "moveTime", node: selected }),
      enabled: !!state.document?.documentId && !e.busy && selected !== 0,
    },
    "insert-diagram": {
      run: () =>
        e.edit({
          kind: "setExtra",
          node: selected,
          extra: {
            kind: "diagram",
            on: !game?.nodes[selected]?.extras?.diagram,
          },
        }),
      enabled: !!state.document?.documentId && !e.busy,
      pressed: !!game?.nodes[selected]?.extras?.diagram,
    },
    "pawn-structure": {
      run: () =>
        e.edit({
          kind: "setExtra",
          node: selected,
          extra: {
            kind: "pawnStructure",
            on: !game?.nodes[selected]?.extras?.pawnStructure,
          },
        }),
      enabled: !!state.document?.documentId && !e.busy && selected !== 0,
      pressed: !!game?.nodes[selected]?.extras?.pawnStructure,
    },
    "piece-path": {
      run: () =>
        e.edit({
          kind: "setExtra",
          node: selected,
          extra: {
            kind: "piecePath",
            on: !game?.nodes[selected]?.extras?.piecePath,
          },
        }),
      enabled: !!state.document?.documentId && !e.busy && selected !== 0,
      pressed: !!game?.nodes[selected]?.extras?.piecePath,
    },
    "analyse-threat": {
      run: () => k.toggleThreat(),
      enabled: !!k.current?.enabled && !k.current.busy,
      pressed: !!k.current?.threat,
    },
    "next-best": {
      run: () => k.leaveOutBest("next"),
      enabled: !!k.current?.enabled && !!k.current.lines.length,
    },
    "search-better": {
      run: () => k.leaveOutBest("better"),
      enabled: !!k.current?.enabled && !!k.current.lines.length,
    },
    "search-clearly-better": {
      run: () => k.leaveOutBest("clearly"),
      enabled: !!k.current?.enabled && !!k.current.lines.length,
    },
    "lock-engine": {
      run: () => k.toggleLock(),
      enabled: !!k.current?.enabled && !!state.document,
      pressed: !!k.current?.locked,
    },
    "goto-lock": {
      run: () => {
        const locked = k.current?.locked;
        if (locked) void w.selectNode(locked.nodeId);
      },
      enabled:
        !!k.current?.locked &&
        k.current.locked.documentId === state.document?.documentId,
    },
    "paste-engine-variation": {
      run: () => pasteEngine(true),
      enabled: !busy && !!bestLine(),
    },
    "paste-engine-move": {
      run: () => pasteEngine(false),
      enabled: !busy && !!bestLine(),
    },
    "load-previous-game": {
      run: () => step(-1),
      enabled:
        !!state.database &&
        !!state.document &&
        state.document.summary.ordinal !== "1",
    },
    "load-next-game": {
      run: () => step(1),
      enabled:
        !!state.database &&
        !!state.document &&
        BigInt(state.document.summary.ordinal) <
          BigInt(state.database.entryCount),
    },
    "text-after": {
      run: () => focusText("after"),
      enabled: !!game?.nodes[selected],
    },
    "text-before": {
      run: () => focusText("before"),
      enabled: !!game && selected !== 0,
    },
    "new-game": { run: () => e.run(() => e.newGame(), false) },
    "new-board-window": { run: () => e.run(() => e.newGame(), false) },
    options: { run: () => setOptionsOpen(true) },
    "position-setup": {
      run: () => {
        setSetupError(undefined);
        setSetupOpen(true);
      },
    },
    undo: { run: () => e.edit({ kind: "undo" }), enabled: !!game && !busy },
    redo: { run: () => e.edit({ kind: "redo" }), enabled: !!game && !busy },
    "delete-variation": {
      run: () => e.edit({ kind: "deleteVariation", node: selected }),
      enabled: inVariation && !busy,
    },
    "null-move": {
      run: () => e.edit({ kind: "appendMove", parent: selected, uci: "0000" }),
      enabled: !!game && !busy,
    },
    "insert-variation": {
      run: () => e.edit({ kind: "insertVariation", node: selected }),
      enabled: !!game && !busy && inVariation,
    },
    "fold-all": {
      run: () =>
        setFolded(
          game?.nodes.filter((n) => n.children.length > 1).map((n) => n.id) ??
            [],
        ),
      enabled: !!game?.nodes.some((n) => n.children.length > 1),
    },
    "unfold-all": { run: () => setFolded([]), enabled: folded.size > 0 },
    "fold-next-node": {
      run: () => {
        const n = nextBranch((id) => !folded.has(id));
        if (n !== undefined) setFold(n, true);
      },
      enabled: !!game,
    },
    "unfold-next-node": {
      run: () => {
        const n = nextBranch((id) => folded.has(id));
        if (n !== undefined) setFold(n, false);
      },
      enabled: folded.size > 0,
    },
    "delete-all-commentary": {
      run: () =>
        e.edit({ kind: "deleteCommentary", commentary: "all", node: selected }),
      enabled: !!game && !busy,
    },
    "delete-text-commentary": {
      run: () =>
        e.edit({
          kind: "deleteCommentary",
          commentary: "text",
          node: selected,
        }),
      enabled: !!game && !busy,
    },
    "delete-evaluation-commentary": {
      run: () =>
        e.edit({
          kind: "deleteCommentary",
          commentary: "evaluationAndTime",
          node: selected,
        }),
      enabled: !!game && !busy,
    },
    "delete-colour-commentary": {
      run: () => e.edit({ kind: "clearMarks" }),
      enabled:
        !busy &&
        !!game?.nodes.some((n) => n.squares?.length || n.arrows?.length),
    },
    "promote-variation": {
      run: () => e.edit({ kind: "promoteVariation", node: selected }),
      enabled: inVariation && !busy,
    },
    "exchange-moves": {
      run: () => e.edit({ kind: "exchangeMoves", node: selected }),
      // The reference app's Exchange Moves works on a variation's first move.
      enabled:
        !busy &&
        !!game &&
        selected !== 0 &&
        (game.nodes[game.nodes[selected]?.parent ?? 0]?.children.indexOf(
          selected,
        ) ?? 0) > 0,
    },
    "reorder-variations": {
      run: () => setReordering(true),
      enabled: !!game && !busy,
    },
    "cleanup-variations": {
      run: () => e.edit({ kind: "cleanUpVariations" }),
      enabled: !!game && !busy,
    },
    "delete-remaining-moves": {
      run: () => e.edit({ kind: "deleteRemaining", node: selected }),
      enabled: !!game?.nodes[selected]?.children.length && !busy,
    },
    "delete-previous-moves": {
      run: () => e.edit({ kind: "deletePrevious", node: selected }),
      enabled: !!game && selected !== 0 && !busy,
    },
    "edit-game-data": {
      run: () => setGameDataOpen(true),
      enabled: !!game && !busy,
    },
    "annotation-palette": {
      run: () => setPaletteOpen((o) => !o),
      enabled: !!game,
      pressed: paletteOpen,
    },
    "symbol-menu-move": {
      run: () => setSymbolMenu("move"),
      enabled: !!game && selected !== 0 && !busy,
    },
    "symbol-menu-evaluation": {
      run: () => setSymbolMenu("position"),
      enabled: !!game && selected !== 0 && !busy,
    },
    "symbol-menu-prefix": {
      run: () => setSymbolMenu("prefix"),
      enabled: !!game && selected !== 0 && !busy,
    },
    "copy-game": { run: () => e.copyGame(), enabled: !!game },
    "paste-game": { run: () => paste("game") },
    "paste-position": { run: () => paste("position") },
    "layout-standard": {
      run: () => setLayout("standard"),
      pressed: layout === "standard",
    },
    "layout-wide-list": {
      run: () => setLayout("wide-list"),
      pressed: layout === "wide-list",
    },
    "layout-board-only": {
      run: () => setLayout("big-board"),
      pressed: layout === "big-board",
    },
    "layout-tall-engine": {
      run: () => setLayout("big-engine"),
      pressed: layout === "big-engine",
    },
    "theme-white": { run: () => setTheme("white"), pressed: theme === "white" },
    "theme-blue": { run: () => setTheme("blue"), pressed: theme === "blue" },
    "theme-gray": { run: () => setTheme("gray"), pressed: theme === "gray" },
    "theme-black": { run: () => setTheme("black"), pressed: theme === "black" },
    "command-search": { run: () => setPalette(true) },
    "tablebase-annotation": {
      run: () => runTablebaseAnnotation(),
      enabled: !!game && !busy,
    },
    "stop-process": {
      run: () =>
        w.cloudDownload
          ? w.cancelCloudDownload()
          : w.searching
            ? w.cancelSearch()
            : w.treeBuild
              ? w.cancelTree()
              : k.stopAll(),
      enabled:
        k.kibitzers.some((v) => v.enabled) ||
        !!w.treeBuild ||
        !!w.searching ||
        !!w.cloudDownload,
    },
  };
  // A move chosen inside folded variations unfolds them.
  useEffect(() => {
    if (!game || !folded.size) return;
    const open: number[] = [];
    for (let id = selected, i = 0; id !== 0 && i < game.nodes.length; i++) {
      const parent = game.nodes[id]?.parent;
      if (parent === undefined || parent === null) break;
      if (folded.has(parent) && game.nodes[parent].children[0] !== id)
        open.push(parent);
      id = parent;
    }
    if (open.length) setFolded([...folded].filter((f) => !open.includes(f)));
  }, [selected, game]);
  // A symbol popup belongs to the move it was opened for.
  useEffect(() => setSymbolMenu(undefined), [selected, game]);
  useEffect(() => {
    if (!startupCommand) return;
    setStartupCommand(undefined);
    // "a,b" runs a, then b with the bindings as they are after a.
    const [first, ...rest] = startupCommand.split(",");
    void (async () => {
      await bindings[first]?.run();
      if (rest.length) {
        await new Promise((r) => setTimeout(r, 2000));
        setStartupCommand(rest.join(","));
        return;
      }
      // Let the result render before the picture is taken.
      await new Promise((r) => setTimeout(r, 500));
      await api.startupShown().catch(() => {});
    })();
  }, [startupCommand]);
  useShortcuts(
    view,
    bindings,
    !!e.question ||
      palette ||
      !!symbolMenu ||
      !!maskOpen ||
      !!playingStyle ||
      !!replay ||
      gameDataOpen ||
      setupOpen ||
      !!pasting ||
      promotion.length > 0,
  );
  // "/" also opens command search when focus is not in a field.
  useEffect(() => {
    function slash(event: KeyboardEvent) {
      if (event.key !== "/" || palette || replay) return;
      if ((event.target as HTMLElement).closest?.("input,textarea,select"))
        return;
      event.preventDefault();
      setPalette(true);
    }
    window.addEventListener("keydown", slash);
    return () => window.removeEventListener("keydown", slash);
  }, [palette, replay]);

  const databaseName = state.database ? "Database" : "Databases";
  /** Open databases in the order they were opened. */
  const openDatabases = [
    ...(state.parked ?? []).map((p) => p.database),
    ...(state.database ? [state.database] : []),
  ].sort(
    (a, b) =>
      Number(a.databaseId.replace(/\D/g, "")) -
      Number(b.databaseId.replace(/\D/g, "")),
  );
  const engineStatus = k.current
    ? `${k.current.engine.identity.name ?? "UCI engine"} · ${k.current.enabled ? "analysing" : "idle"}${
        k.kibitzers.length > 1 ? ` (+${k.kibitzers.length - 1})` : ""
      }`
    : "No engine";

  const pgnSelect = e.collection && (
    <label className="pgn-select">
      PGN game
      <select
        aria-label="PGN game"
        value={
          state.document?.metadataStatus === "available" &&
          state.document?.annotationsStatus === "available"
            ? Number(state.document.summary.ordinal) - 1
            : ""
        }
        onChange={(event) => {
          const index = Number(event.target.value);
          void e.run(() => e.loadPgn(index));
        }}
      >
        <option value="" disabled>
          Select game
        </option>
        {e.collection.games.map((tags, i) => (
          <option key={i} value={i}>
            {i + 1}. {tags.White ?? "?"} — {tags.Black ?? "?"}
          </option>
        ))}
      </select>
    </label>
  );

  const notation = game ? (
    <>
      {settings.evalProfileNotation && profileView(true)}
      <NotationTree
        nodes={game.nodes}
        selection={selected}
        onSelect={(id) => void w.selectNode(id)}
        startFen={game.startFen}
        folded={folded}
        onUnfold={(id) => setFold(id, false)}
        onTraining={(id) => {
          void w.selectNode(id);
          setTraining({ node: id });
        }}
      />
    </>
  ) : (
    <div className="empty-state">
      Select a game to replay its moves and variations.
    </div>
  );

  return (
    <div className="app" data-theme={theme}>
      <header className="title-bar">
        <div className="brand" aria-hidden>
          <LayoutGrid size={16} />
        </div>
        <nav className="workspace-tabs" role="tablist" aria-label="Workspace">
          {openDatabases.length === 0 && (
            <button
              role="tab"
              aria-selected={view === "database"}
              className="workspace-tab"
              onClick={() => setView("database")}
            >
              <Database size={14} aria-hidden />
              <span>{databaseName}</span>
            </button>
          )}
          {openDatabases.map((db) => {
            const active = db.databaseId === state.database?.databaseId;
            return (
              <span
                key={db.databaseId}
                className="workspace-tab-wrap"
                data-selected={view === "database" && active}
              >
                <button
                  role="tab"
                  aria-selected={view === "database" && active}
                  aria-label={`Database ${db.name ?? ""}`.trim()}
                  className="workspace-tab"
                  onClick={() => {
                    if (!active) w.switchDatabase(db.databaseId);
                    setView("database");
                  }}
                >
                  <Database size={14} aria-hidden />
                  <span>{db.name || databaseName}</span>
                  <small>{db.entryCount}</small>
                  {db.writable && <small title="Opened for editing">✎</small>}
                </button>
                <button
                  className="tab-close"
                  aria-label={`Close database ${db.name ?? ""}`.trim()}
                  title="Close database"
                  disabled={busy}
                  onClick={() => w.closeDatabase(db.databaseId)}
                >
                  <X size={12} />
                </button>
              </span>
            );
          })}
          {tabGames(state).map((tab) => {
            const shown = tab.id === state.focus;
            const tags = tab.document?.game?.tags;
            const name = tags
              ? `${tags.White ?? "?"} — ${tags.Black ?? "?"}`
              : "Game";
            return (
              <span
                key={tab.id}
                className="workspace-tab-wrap"
                data-selected={shown}
              >
                <button
                  role="tab"
                  aria-selected={shown}
                  className="workspace-tab"
                  title={name}
                  onClick={() => e.showTab(tab.id)}
                >
                  <LayoutGrid size={14} aria-hidden />
                  <span>{name}</span>
                  {tab.dirty && (
                    <span className="dirty-dot" title="Unsaved changes">
                      ●
                    </span>
                  )}
                </button>
                <button
                  className="tab-close"
                  aria-label={`Close game tab ${name}`}
                  title="Close game"
                  disabled={busy}
                  onClick={() => void e.closeTab(tab.id)}
                >
                  <X size={12} />
                </button>
              </span>
            );
          })}
        </nav>
        <button
          className="command-search"
          onClick={() => setPalette(true)}
          title="Search commands"
        >
          <Search size={14} aria-hidden />
          <span>Search commands</span>
          <kbd>/</kbd>
        </button>
      </header>
      <Ribbon view={view} bindings={bindings} />
      <main className="workspace">
        {startShown ? (
          <div className="start-wrap">
            <Diagnostics items={w.diagnostics} />
            <StartScreen
              api={api}
              showFolders={showFolders}
              view={startView}
              onView={changeStartView}
              onProperties={setPropertySymbol}
              onSelect={setStartSymbol}
              refresh={startRefresh}
              onOpen={(path) =>
                void w.openDatabase(path).then(() => {
                  setStartOpen(false);
                  setStartRefresh((n) => n + 1);
                })
              }
            />
          </div>
        ) : view === "database" ? (
          <Group
            key={`db-${layout}`}
            orientation="horizontal"
            className="split"
          >
            <Panel
              id="list"
              defaultSize={layout === "wide-list" ? "75" : "58"}
              minSize="30"
            >
              <section className="pane list-pane" aria-label="Database">
                <div className="list-tabs" role="tablist" aria-label="Lists">
                  {(
                    [
                      ["games", "Games"],
                      ["players", "Players"],
                      ["tournaments", "Tournaments"],
                    ] as const
                  ).map(([tab, label]) => (
                    <button
                      key={tab}
                      role="tab"
                      aria-selected={listTab === tab}
                      disabled={tab !== "games" && !state.database}
                      onClick={() => showList(tab)}
                    >
                      {label}
                    </button>
                  ))}
                </div>
                <div className="pane-header" hidden={listTab !== "games"}>
                  <strong>
                    {state.search?.label
                      ? `${state.search.label}: ${state.search.hits} games`
                      : state.search
                        ? `${state.search.hits} of ${state.database?.entryCount} entries found`
                        : state.database
                          ? `${state.database.entryCount} entries`
                          : "No database open"}
                  </strong>
                  {state.search && (
                    <button
                      className="link-button"
                      onClick={() => void w.clearSearch()}
                    >
                      Reset filter
                    </button>
                  )}
                  {pgnSelect}
                  {state.dirty && (
                    <span className="badge">Unsaved changes</span>
                  )}
                  {pastingGames && (
                    <span className="sorting">
                      Pasting: {pastingGames}
                      <button
                        className="link-button"
                        onClick={() =>
                          state.database &&
                          void api.cancelPaste(state.database.databaseId)
                        }
                      >
                        Cancel
                      </button>
                    </span>
                  )}
                  {selectedCount(selection) > 1 && (
                    <span className="hint">
                      {selectedCount(selection).toLocaleString()} selected
                    </span>
                  )}
                  {w.sorting && (
                    <span className="sorting">
                      Sorting: {w.sorting}
                      <button
                        className="link-button"
                        onClick={() => w.cancelSort()}
                      >
                        Cancel
                      </button>
                    </span>
                  )}
                </div>
                {listTab === "games" && (filterOpen || filter) && (
                  <form
                    className="filter-bar"
                    onSubmit={(ev) => {
                      ev.preventDefault();
                      void w.search({
                        ...(state.search?.filter ?? emptyFilter()),
                        text: filter,
                      });
                    }}
                  >
                    <input
                      aria-label="Filter games"
                      placeholder="Try Kasparov- B10-B17, Site:London, >2400…"
                      value={filter}
                      autoFocus
                      onChange={(ev) => setFilter(ev.target.value)}
                      onKeyDown={(ev) => {
                        if (ev.key === "Escape") {
                          setFilter("");
                          setFilterOpen(false);
                        }
                      }}
                    />
                    <span
                      className="hint"
                      title={
                        'Combine terms; -Karpov means Black, 2001-2011 is a year range, Event:"World Championship" matches a phrase. Positive results such as 1-0 and 0-1 are alternatives.'
                      }
                    >
                      Enter searches all games · Use ! to exclude a term
                    </span>
                  </form>
                )}
                {listTab === "games" && (
                  <GameList
                    rowCount={w.rowCount}
                    version={`${state.database?.databaseId}:${state.listVersion}`}
                    row={(i) => w.rows.row(i)}
                    want={(from, to) => w.rows.want(from, to, w.rowCount)}
                    top={w.listTop}
                    onTop={w.setListTop}
                    onMeasure={setVisibleRows}
                    cursor={w.cursor}
                    selected={state.document?.summary.ordinal}
                    loading={w.loadingEntry}
                    sort={state.sort}
                    sorting={!!w.sorting}
                    onSort={(s) => void w.sortBy(s?.column, s?.descending)}
                    onPreview={preview}
                    onOpen={(row) => void open(row)}
                    isSelected={
                      selectedCount(selection) > 1
                        ? (row) => isSelected(selection, row)
                        : undefined
                    }
                  />
                )}
                {state.database &&
                  openedTabs.map((kind) => (
                    <div
                      key={`${state.database!.databaseId}:${kind}`}
                      className="names-host"
                      hidden={listTab !== kind}
                    >
                      <NamesPane
                        api={api}
                        databaseId={state.database!.databaseId}
                        kind={kind}
                        onChoose={(row) =>
                          setChosenName((c) => ({ ...c, [kind]: row }))
                        }
                        onGames={(row) => void gamesOf(kind, row)}
                      />
                    </div>
                  ))}
                {listTab === "games" && (
                  <div className="pager">
                    <span className="list-position">
                      {w.rowCount
                        ? `Rows ${(w.listTop + 1).toLocaleString()}–${Math.min(
                            w.rowCount,
                            w.listTop + visibleRows,
                          ).toLocaleString()} of ${w.rowCount.toLocaleString()}`
                        : ""}
                    </span>
                    <label>
                      Entry number{" "}
                      <input
                        value={ordinal}
                        onChange={(ev) => setOrdinal(ev.target.value)}
                        onKeyDown={(ev) => ev.key === "Enter" && void jump()}
                        inputMode="numeric"
                      />
                    </label>
                    <button
                      onClick={() => void jump()}
                      disabled={!state.database}
                    >
                      Go to entry
                    </button>
                  </div>
                )}
              </section>
            </Panel>
            <Separator className="splitter vertical" />
            <Panel id="preview" minSize="25">
              <section className="pane preview-pane" aria-label="Preview">
                <BoardPane
                  title={title}
                  position={state.position}
                  flipped={flipped}
                  hasGame={!!game}
                  navigate={navigate}
                  compact
                  node={boardMarks(game, selected, state.position?.fen)}
                />
                <div className="preview-notation">{notation}</div>
                <Diagnostics items={w.diagnostics} />
                {game && (
                  <button className="open-game" onClick={() => setView("game")}>
                    Open game <kbd>{format("Enter")}</kbd>
                  </button>
                )}
              </section>
            </Panel>
          </Group>
        ) : (
          <Group
            key={`game-${layout}`}
            orientation="vertical"
            className="split"
          >
            <Panel
              id="top"
              defaultSize={layout === "big-engine" ? "55" : "74"}
              minSize="30"
            >
              <Group orientation="horizontal" className="split">
                <Panel
                  id="board"
                  defaultSize={layout === "big-board" ? "68" : "52"}
                  minSize="30"
                >
                  <section className="pane" aria-label="Board">
                    <BoardPane
                      title={title}
                      position={state.position}
                      flipped={flipped}
                      hasGame={!!game}
                      navigate={navigate}
                      node={boardMarks(game, selected, state.position?.fen)}
                      selectedSquare={square}
                      onSquare={promotion.length ? undefined : clickSquare}
                      onMove={
                        promotion.length
                          ? undefined
                          : (from, to) => clickSquare(to, from)
                      }
                      onMark={drawMark}
                      promotion={promotion}
                      onPromote={(uci) => {
                        if (uci && activePractice) void gradePractice(uci);
                        else if (uci)
                          void e.edit({
                            kind: "appendMove",
                            parent: selected,
                            uci,
                          });
                        setPromotion([]);
                      }}
                    />
                  </section>
                </Panel>
                <Separator className="splitter vertical" />
                <Panel id="notation" minSize="20">
                  <section className="pane notation-pane" aria-label="Notation">
                    <div className="pane-header">
                      <div
                        className="pane-tabs"
                        role="tablist"
                        aria-label="Notation tabs"
                      >
                        {(
                          [
                            ["notation", "Notation"],
                            ["book", "Book"],
                            ["reference", "Reference"],
                            ["repertoire", "Repertoire"],
                            ["tablebase", "Tablebase"],
                          ] as const
                        ).map(([tab, label]) => (
                          <button
                            key={tab}
                            role="tab"
                            aria-selected={notationTab === tab}
                            onClick={() => setNotationTab(tab)}
                          >
                            {label}
                          </button>
                        ))}
                      </div>
                      {pgnSelect}
                      {state.dirty && (
                        <span className="badge">Unsaved changes</span>
                      )}
                    </div>
                    {symbolMenu && game?.nodes[selected] && selected !== 0 && (
                      <SymbolMenu
                        slot={symbolMenu}
                        node={game.nodes[selected]}
                        onChoose={(nags) => {
                          setSymbolMenu(undefined);
                          void e.edit({
                            kind: "setNags",
                            node: selected,
                            nags,
                          });
                        }}
                        onClose={() => setSymbolMenu(undefined)}
                      />
                    )}
                    {paletteOpen && (
                      <AnnotationPalette
                        node={boardMarks(game, selected, state.position?.fen)}
                        disabled={busy}
                        edit={e.edit}
                        onClose={() => setPaletteOpen(false)}
                      />
                    )}
                    {notationTab === "book" && bookPane}
                    {notationTab === "reference" && referencePane}
                    {notationTab === "repertoire" && repertoirePane}
                    {notationTab === "tablebase" && (
                      <TablebasePane
                        api={api}
                        fen={state.position?.fen}
                        onPlay={playBookMove}
                      />
                    )}
                    {notationTab === "notation" && (
                      <div className="notation-scroll">{notation}</div>
                    )}
                    {notationTab === "notation" && game?.nodes[selected] && (
                      <CommentEditor
                        key={commentFocus ? `focus-${commentFocus}` : "editor"}
                        node={game.nodes[selected]}
                        disabled={busy}
                        edit={e.edit}
                        autoFocus={commentFocus > 0 ? focusField : undefined}
                      />
                    )}
                    <Diagnostics items={w.diagnostics} />
                    {annotationsIncomplete(
                      state.document?.annotationsStatus,
                    ) && (
                      <p className="compatibility-note">
                        Some original database annotations are not shown.
                      </p>
                    )}
                  </section>
                </Panel>
              </Group>
            </Panel>
            <Separator
              className="splitter horizontal"
              aria-label="Resize analysis pane"
            />
            <Panel id="engine" minSize="12">
              <Group orientation="horizontal" className="split">
                <Panel id="kibitzer" minSize="20">
                  <section className="pane analysis-pane" aria-label="Engine">
                    {settings.evalProfilePane && profileView(false)}
                    <EnginePanel k={k} state={state} />
                  </section>
                </Panel>
                {extraBook && (
                  <>
                    <Separator className="splitter vertical" />
                    <Panel id="extra-book" minSize="20">
                      <section className="pane" aria-label="Book pane">
                        {bookPane}
                      </section>
                    </Panel>
                  </>
                )}
                {extraReference && (
                  <>
                    <Separator className="splitter vertical" />
                    <Panel id="extra-reference" minSize="20">
                      <section className="pane" aria-label="Reference pane">
                        {referencePane}
                      </section>
                    </Panel>
                  </>
                )}
              </Group>
            </Panel>
          </Group>
        )}
      </main>
      <footer className="status-bar">
        <span className="app-status">
          {w.status}
          {w.searching && (
            <button
              type="button"
              className="link-button"
              onClick={() => w.cancelSearch()}
            >
              Cancel
            </button>
          )}
        </span>
        <span>{game ? `${title} · move ${selected}` : ""}</span>
        <span>{engineStatus}</span>
      </footer>
      {palette && (
        <CommandPalette
          view={view}
          bindings={bindings}
          onClose={() => setPalette(false)}
        />
      )}
      <UnsavedDialog question={e.question} answer={e.answer} />
      {recovery && (
        <RecoveryDialog
          api={api}
          initialSource={recovery.source}
          onClose={() => setRecovery(undefined)}
          onOpen={async (path) => {
            await w.openDatabase(path);
            setStartOpen(false);
            setStartRefresh((n) => n + 1);
          }}
        />
      )}
      {propertySymbol && (
        <DatabasePropertiesDialog
          symbol={propertySymbol}
          onClose={() => setPropertySymbol(undefined)}
          save={async (properties) => {
            await api.setDatabaseProperties(propertySymbol.path, properties);
            setStartRefresh((n) => n + 1);
          }}
        />
      )}
      {gameDataOpen && game && (
        <GameDataDialog
          tags={game.tags}
          onClose={() => setGameDataOpen(false)}
          onApply={(tags) =>
            void e.edit({ kind: "setTags", tags }).then((r) => {
              if (r !== null) setGameDataOpen(false);
            })
          }
        />
      )}
      {optionsOpen && (
        <OptionsDialog
          settings={settings}
          error={optionsError}
          chooseFolder={api.chooseTablebaseFolder}
          onClose={() => setOptionsOpen(false)}
          onApply={(next) =>
            void api
              .setSettings(next)
              .then((saved) => {
                setSettings(saved);
                setOptionsOpen(false);
                setOptionsError(undefined);
              })
              .catch((err) => setOptionsError(explain(err)))
          }
        />
      )}
      {selEdit && (
        <SelectionDataDialog
          games={selEdit.games}
          initial={selEdit.initial}
          running={selEdit.running}
          error={selEdit.error}
          onStart={(names) => void runSelectionEdit(names)}
          onCancel={() =>
            void api.cancelMaintenance(selEdit.databaseId).catch(() => {})
          }
          onClose={() => setSelEdit(undefined)}
        />
      )}
      {job?.kind === "doubles" && job.options && (
        <DoublesDialog
          running={job.running}
          error={job.error}
          onStart={(o) => void runDoubles(o)}
          onCancel={() =>
            void api.cancelMaintenance(job.databaseId).catch(() => {})
          }
          onClose={() => setJob(undefined)}
        />
      )}
      {job && !job.options && (
        <JobDialog
          title={
            {
              integrity: "Check Integrity",
              doubles: "Find Double Games",
              compact: "Remove Deleted Games",
              eco: "Set ECO Codes",
            }[job.kind]
          }
          running={job.running && `${job.running}`}
          error={job.error}
          lines={job.lines}
          onCancel={() =>
            void api.cancelMaintenance(job.databaseId).catch(() => {})
          }
          onClose={() => setJob(undefined)}
        />
      )}
      {converting && (
        <ConvertDialog
          name={converting.name}
          entries={converting.entries}
          selected={converting.selected}
          scope={converting.scope}
          target={converting.target}
          estimate={converting.estimate}
          running={converting.running}
          error={converting.error}
          onScope={(scope) => {
            const next = { ...converting, scope, estimate: undefined };
            setConverting(next);
            void estimateConvert(next);
          }}
          onTarget={() =>
            void api.chooseConvertTarget(converting.name).then((target) => {
              if (typeof target !== "string") return;
              const next = { ...converting, target, estimate: undefined };
              setConverting(next);
              void estimateConvert(next);
            })
          }
          onStart={() => void runConvert()}
          onCancel={() =>
            void api.cancelConvert(converting.databaseId).catch(() => {})
          }
          onClose={() => setConverting(undefined)}
        />
      )}
      {blunder && (
        <BlunderCheckDialog
          engines={settings.engines}
          defaultEngine={settings.defaultEngine}
          running={blunder.running}
          error={blunder.error}
          onChooseFile={api.pickEngine}
          onStart={(params, store) => void runBlunderCheck(params, store)}
          onCancel={() => {
            if (blunder.documentId)
              void api.cancelBlunderCheck(blunder.documentId).catch(() => {});
          }}
          onClose={() => setBlunder(undefined)}
        />
      )}
      {training && game?.nodes[training.node] && (
        <TrainingDialog
          node={game.nodes[training.node]}
          moveText={moveLabel(game.nodes, training.node)}
          error={training.error}
          onSave={(t, moves) =>
            void e
              .edit({
                kind: "setTraining",
                node: training.node,
                training: t,
                moves,
              })
              .then((result) =>
                result === null
                  ? setTraining({
                      ...training,
                      error:
                        "The question could not be saved: check the answers' moves.",
                    })
                  : setTraining(undefined),
              )
          }
          onClose={() => setTraining(undefined)}
        />
      )}
      {replay && (
        <ReplayTrainingSession
          api={api}
          source={replay.source}
          onClose={() => setReplay(undefined)}
        />
      )}
      {novelty && (
        <NoveltyDialog
          report={novelty.report}
          running={novelty.running}
          error={novelty.error}
          databaseName={state.database?.name ?? "the database"}
          moveName={(ply) =>
            plyName(
              ply,
              game?.nodes[lineIds[ply]]?.san ?? "",
              game?.startFen.split(" ")[1] !== "b",
            )
          }
          onSelectNovelty={() => {
            const node = novelty.report?.novelty?.node;
            if (node !== undefined) void w.selectNode(node);
          }}
          onStop={() => void api.cancelFindNovelty()}
          onClose={() => setNovelty(undefined)}
        />
      )}
      {extraDialog?.kind === "medal" && (
        <MedalDialog
          bits={game?.nodes[extraDialog.node]?.extras?.medal ?? 0}
          onClose={() => setExtraDialog(undefined)}
          onApply={(bits) => {
            const node = extraDialog.node;
            setExtraDialog(undefined);
            void e.edit({
              kind: "setExtra",
              node,
              extra: { kind: "medal", bits },
            });
          }}
        />
      )}
      {extraDialog?.kind === "moveTime" && (
        <MoveTimeDialog
          time={game?.nodes[extraDialog.node]?.extras?.moveTime}
          onClose={() => setExtraDialog(undefined)}
          onApply={(time) => {
            const node = extraDialog.node;
            setExtraDialog(undefined);
            void e.edit({
              kind: "setExtra",
              node,
              extra: { kind: "moveTime", time },
            });
          }}
        />
      )}
      {centipawnList && (
        <CentipawnListDialog
          api={api}
          databaseId={centipawnList.databaseId}
          databaseName={centipawnList.name}
          ranges={centipawnList.ranges}
          selected={centipawnList.selected}
          engines={settings.engines}
          defaultEngine={settings.defaultEngine}
          onChooseFile={api.pickEngine}
          onDone={() => void w.refreshList()}
          onClose={() => setCentipawnList(undefined)}
        />
      )}
      {centipawn && (
        <CentipawnDialog
          api={api}
          documentId={centipawn}
          engines={settings.engines}
          defaultEngine={settings.defaultEngine}
          onChooseFile={api.pickEngine}
          initialReport={
            gameReview?.documentId === centipawn &&
            gameReview.line === mainLineKey(game)
              ? gameReview.report
              : undefined
          }
          game={state.document?.documentId === centipawn ? game : undefined}
          onSelectNode={(node) => void w.selectNode(node)}
          onApply={async (report) => {
            const now = w.current.current.document;
            if (
              now?.documentId !== centipawn ||
              now.revision !== report.revision
            )
              return "The game changed during the analysis; run it again.";
            const result = await e.edit({
              kind: "addAnalysis",
              items: report.items.map((i) => ({
                node: i.node,
                comment: i.comment,
                line: [],
              })),
            });
            if (result === null) return "The texts could not be added.";
            setGameReview({
              documentId: centipawn,
              line: mainLineKey(now.game),
              report,
            });
            w.setStatus(
              `Centipawn Analysis: average loss White ${report.white.averageLoss?.toFixed(0) ?? "–"}, Black ${report.black.averageLoss?.toFixed(0) ?? "–"}`,
            );
            return undefined;
          }}
          onClose={() => setCentipawn(undefined)}
        />
      )}
      {job?.kind === "eco" && job.options && (
        <SetEcoDialog
          onStart={(onlyMissing) => void runSetEco(onlyMissing)}
          onClose={() => setJob(undefined)}
        />
      )}
      {report && (
        <OpeningReportDialog
          report={report.result}
          startFen={report.startFen}
          running={report.running}
          error={report.error}
          onStop={() =>
            void api.cancelMaintenance(report.databaseId).catch(() => {})
          }
          onClose={() => setReport(undefined)}
        />
      )}
      {reordering && (
        <ReorderDialog
          onAnswer={(mainLine) => {
            setReordering(false);
            void e.edit({
              kind: "reorderVariations",
              node: selected,
              mainLine,
            });
          }}
          onClose={() => setReordering(false)}
        />
      )}
      {classified && (
        <ClassifyDialog
          result={classified}
          onSetEco={
            classified.eco && classified.eco !== classified.current && game
              ? () => {
                  const eco = classified.eco!;
                  setClassified(undefined);
                  void e.edit({
                    kind: "setTags",
                    tags: { ...game.tags, ECO: eco },
                  });
                }
              : undefined
          }
          onClose={() => setClassified(undefined)}
        />
      )}
      {deep && (
        <DeepAnalysisDialog
          engines={settings.engines}
          defaultEngine={settings.defaultEngine}
          running={deep.running}
          error={deep.error}
          onChooseFile={api.pickEngine}
          onStart={(params) => void runDeepAnalysis(params)}
          onStop={(discard) => {
            if (deep.documentId)
              void api
                .stopDeepAnalysis(deep.documentId, discard)
                .catch(() => {});
          }}
          onClose={() => setDeep(undefined)}
        />
      )}
      {jobs && (
        <AnalysisJobsDialog
          view={jobs.view}
          engines={settings.engines}
          defaultEngine={settings.defaultEngine}
          kind={jobKind}
          progress={jobs.progress}
          error={jobs.error}
          onKindChange={setJobKind}
          onChooseFile={api.pickEngine}
          onRun={(params) => void runJobs(params)}
          onStop={() => void api.stopAnalysisJobs().catch(() => {})}
          onEdit={(id, action) => void editJob(id, action)}
          onClose={() => setJobs(undefined)}
        />
      )}
      {enginesOpen && (
        <EngineManagement
          settings={settings}
          error={enginesError}
          onClose={() => {
            setEnginesOpen(false);
            setEnginesError(undefined);
          }}
          onSave={(next) =>
            void saveSettings(next)
              .then(() => setEnginesError(undefined))
              .catch((err) => setEnginesError(explain(err)))
          }
          onAdd={() =>
            void (async () => {
              const path = await api.pickEngine();
              if (typeof path !== "string") return;
              try {
                const identity = await api.probeEngine(path);
                const name =
                  identity.name ?? path.split(/[\\/]/).pop() ?? "UCI engine";
                const id = `engine-${Date.now().toString(36)}`;
                await saveSettings({
                  ...settings,
                  engines: [
                    ...settings.engines,
                    { id, name, path, options: {} },
                  ],
                  defaultEngine: settings.defaultEngine ?? id,
                });
                setEnginesError(undefined);
              } catch (err) {
                setEnginesError(explain(err));
              }
            })()
          }
          onStart={(engine) => {
            setEnginesOpen(false);
            void k.add(engine.path, engine);
          }}
        />
      )}
      {setupOpen && (
        <PositionSetup
          boardFen={state.position?.fen}
          error={setupError}
          onClose={() => setSetupOpen(false)}
          onApply={(fen) =>
            void e.run(async () => {
              try {
                await e.newGame(fen);
                setSetupOpen(false);
              } catch (err) {
                setSetupError(
                  (err as { message?: string })?.message ?? String(err),
                );
              }
            }, false)
          }
        />
      )}
      {pasting && (
        <PasteDialog
          kind={pasting}
          onClose={() => setPasting(undefined)}
          onPaste={(text) => {
            setPasting(undefined);
            void pasteText(pasting, text);
          }}
        />
      )}
      {stats && (
        <PlayerStatisticsDialog
          stats={stats.data}
          progress={stats.progress}
          error={stats.error}
          players={stats.players}
          onPlayer={(id) => void openStatistics(id)}
          onCancel={() => void api.cancelPlayerStatistics(stats.databaseId)}
          onOpenGames={(group, label) => {
            const playerId = stats.playerId;
            setStats(undefined);
            showList("games");
            void w.statisticsGames(playerId, group, label);
          }}
          onCrosstable={(id) => void openCrosstable(id)}
          onClose={() => {
            if (stats.progress)
              void api.cancelPlayerStatistics(stats.databaseId);
            setStats(undefined);
          }}
        />
      )}
      {playingStyle && (
        <PlayingStyleDialog
          key={`${playingStyle.databaseId}-${playingStyle.player.id}`}
          api={api}
          {...playingStyle}
          onClose={() => setPlayingStyle(undefined)}
          onLoadGame={(ordinal, node, jobId) => {
            setPlayingStyle(undefined);
            setListTab("games");
            void e.openEntry(ordinal, node, jobId);
          }}
        />
      )}
      {dossier && (
        <DossierDialog
          name={dossier.name}
          data={dossier.data}
          progress={dossier.progress}
          error={dossier.error}
          onCancel={() => void api.cancelDossier(dossier.databaseId)}
          onOpenGames={(group, label) => {
            const playerId = dossier.playerId;
            setDossier(undefined);
            showList("games");
            void w.statisticsGames(playerId, group, label);
          }}
          onLoadGame={(ordinal) => {
            setDossier(undefined);
            setListTab("games");
            void e.openEntry(ordinal);
          }}
          onClose={() => {
            if (dossier.progress) void api.cancelDossier(dossier.databaseId);
            setDossier(undefined);
          }}
        />
      )}
      {repertoire && !repertoire.report && (
        <GenerateRepertoireDialog
          api={api}
          databaseId={repertoire.databaseId}
          onGenerated={(result) => {
            for (const side of result.sides)
              if (side.path)
                repertoirePaths.current[side.white ? "white" : "black"] =
                  side.path;
          }}
          onOpen={(path) => {
            setRepertoire(undefined);
            void w.openDatabase(path);
          }}
          onClose={() => setRepertoire(undefined)}
        />
      )}
      {repertoire?.report && (
        <RepertoireReportDialog
          api={api}
          databaseId={repertoire.databaseId}
          white={repertoire.report.white}
          initialPath={
            repertoire.report.white
              ? repertoirePaths.current.white
              : repertoirePaths.current.black
          }
          onListGames={(p) => {
            setRepertoire(undefined);
            showList("games");
            void w.showEntries(p.ordinals, `Repertoire: ${p.text}`);
          }}
          onLoadGame={(ordinal) => {
            setRepertoire(undefined);
            setListTab("games");
            void e.openEntry(ordinal);
          }}
          onClose={() => setRepertoire(undefined)}
        />
      )}
      {prep && (
        <PrepareDialog
          name={prep.name}
          white={prep.white}
          data={prep.data}
          progress={prep.progress}
          error={prep.error}
          onColor={(white) =>
            void openPreparation(prep.playerId, white, prep.name)
          }
          onCancel={() => void api.cancelPreparation(prep.databaseId)}
          onShowInBook={() => {
            const colour = prep.white ? "White" : "Black";
            setBookPrep({
              databaseId: prep.databaseId,
              playerId: prep.playerId,
              white: prep.white,
              label: `Prepare against ${colour}: ${prep.name ?? "?"}`,
            });
            setPrep(undefined);
            if (game) {
              setView("game");
              setNotationTab("book");
            }
          }}
          onListGames={(line) => {
            const { playerId, white, name } = prep;
            const colour = white ? "White" : "Black";
            setPrep(undefined);
            showList("games");
            void w.preparationGames(
              playerId,
              white,
              line ? { startFen: START_FEN, moves: line.moves } : null,
              `${name ?? "?"} as ${colour}${line ? `: ${line.text}` : ""}`,
            );
          }}
          onLoadGame={(ordinal) => {
            setPrep(undefined);
            setListTab("games");
            void e.openEntry(ordinal);
          }}
          onClose={() => {
            if (prep.progress) void api.cancelPreparation(prep.databaseId);
            setPrep(undefined);
          }}
        />
      )}
      {cross && (
        <CrosstableDialog
          table={cross.table}
          loading={cross.loading}
          error={cross.error}
          onLayout={(layout) => void openCrosstable(cross.tournamentId, layout)}
          onOpenGame={(ordinal) => {
            setCross(undefined);
            setListTab("games");
            void e.openEntry(String(ordinal));
          }}
          onReport={() => {
            const id = cross.tournamentId;
            setCross(undefined);
            void openReport(id);
          }}
          onClose={() => setCross(undefined)}
        />
      )}
      {errors && (
        <ErrorReportDialog
          engines={settings.engines}
          defaultEngine={settings.defaultEngine}
          players={errors.players}
          selected={errors.selected}
          report={errors.report}
          running={errors.running}
          error={errors.error}
          onChooseFile={api.pickEngine}
          onStart={(params) => {
            const { databaseId, ranges } = errors;
            setErrors(
              (c) =>
                c && {
                  ...c,
                  running: "Starting the engine…",
                  error: undefined,
                },
            );
            void api
              .errorReport(databaseId, { ...params, ranges }, (p) =>
                setErrors(
                  (c) =>
                    c && {
                      ...c,
                      running: `Game ${Math.min(p.gamesDone + 1, p.games)} of ${p.games} · ${p.positions} positions`,
                    },
                ),
              )
              .then((report) =>
                setErrors((c) => c && { ...c, report, running: undefined }),
              )
              .catch((err) =>
                setErrors(
                  (c) => c && { ...c, running: undefined, error: explain(err) },
                ),
              );
          }}
          onCancel={() => void api.cancelErrorReport(errors.databaseId)}
          onLoadGame={(ordinal, node) => {
            setErrors(undefined);
            setListTab("games");
            void e.openEntry(ordinal, node);
          }}
          onClose={() => setErrors(undefined)}
        />
      )}
      {tourReport && (
        <TournamentReportDialog
          report={tourReport.data}
          loading={tourReport.loading}
          error={tourReport.error}
          onCrosstable={() => {
            const id = tourReport.tournamentId;
            setTourReport(undefined);
            void openCrosstable(id);
          }}
          onListGames={(ordinals, label) => {
            setTourReport(undefined);
            showList("games");
            void w.showEntries(ordinals, label);
          }}
          onLoadGame={(ordinal) => {
            setTourReport(undefined);
            setListTab("games");
            void e.openEntry(ordinal);
          }}
          onClose={() => setTourReport(undefined)}
        />
      )}
      {maskOpen && (
        <SearchMask
          initialTab={maskOpen === "annotations" ? "annotations" : undefined}
          initial={state.search?.filter}
          resultCount={state.search?.hits}
          initialPosition={state.search?.position}
          boardFen={state.position?.fen}
          onSearchPosition={(query, scope) => {
            setMaskOpen(false);
            setView("database");
            void w.positionSearch(query, scope);
          }}
          onClose={() => setMaskOpen(false)}
          onSearch={(filter, scope) => {
            setMaskOpen(false);
            setView("database");
            void w.search(filter, undefined, scope);
          }}
        />
      )}
    </div>
  );
}

/** "2.Nf3" or "2...Nf6" for a node, counting plies from the start. */
function moveLabel(nodes: GameNode[], id: number): string {
  let ply = 0;
  for (
    let at = nodes[id]?.parent;
    at !== undefined && nodes[at]?.parent !== undefined;
    at = nodes[at]?.parent
  )
    ply++;
  const n = nodes[id];
  const number = Math.floor(ply / 2) + 1;
  return `${number}${ply % 2 ? "..." : "."}${n?.san ?? n?.uci ?? ""}`;
}
