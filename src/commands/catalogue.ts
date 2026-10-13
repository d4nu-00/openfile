import {
  Archive,
  ArrowUpToLine,
  BookOpen,
  ChartColumn,
  ChevronsLeft,
  ChevronsRight,
  ClipboardPaste,
  Cloud,
  Columns2,
  Copy,
  Cpu,
  Database,
  FilePlus,
  FileText,
  Funnel as Filter,
  FolderOpen,
  Gauge,
  Globe,
  GraduationCap,
  HardDrive,
  Info,
  Keyboard,
  LayoutGrid,
  LifeBuoy,
  Lightbulb,
  ListTree,
  Mail,
  Maximize2,
  MessageSquareQuote,
  MessageSquareText,
  Microscope,
  Minus,
  Palette,
  PanelsTopLeft,
  Play,
  Plus,
  Redo2,
  RotateCcw,
  Rows2,
  Save,
  Search,
  Settings,
  Split,
  Square,
  Star,
  Swords,
  TableProperties,
  Trash2,
  Undo2,
  Users,
  Wrench,
  X,
  ArrowDownUp,
  type LucideIcon,
} from "lucide-react";

/** A workspace view; each has its own ribbon, as ChessBase's windows do. */
export type ViewId = "database" | "game";

export interface CommandMeta {
  id: string;
  label: string;
  tip: string;
  icon: LucideIcon;
  /** Default shortcut in ChessBase notation ("Ctrl" is ⌘ on macOS). */
  keys?: string;
  /** Views where the shortcut and palette entry apply. */
  views: ViewId[];
}

const both: ViewId[] = ["database", "game"];
const db: ViewId[] = ["database"];
const game: ViewId[] = ["game"];

/*
 * Command names, tooltips and default shortcuts follow ChessBase 18's command
 * table (M_KC_* ids and their M_MN_/M_TX_ messages); the grouping below follows
 * its ribbon builders. Commands added by this workbench are marked "workbench".
 */
const list: CommandMeta[] = [
  // Database window
  {
    id: "open-database",
    label: "Open Database",
    tip: "Open a ChessBase database (.cbh or .2cbh)",
    icon: FolderOpen,
    keys: "Ctrl+O",
    views: both,
  },
  {
    id: "open-pgn",
    label: "Open PGN",
    tip: "Open a PGN file (workbench)",
    icon: FileText,
    views: both,
  },
  {
    id: "new-database",
    label: "New",
    tip: "Create a new database",
    icon: FilePlus,
    keys: "Ctrl+X",
    views: db,
  },
  {
    id: "my-databases",
    label: "My Databases",
    tip: "Show your favourite databases",
    icon: Star,
    views: db,
  },
  {
    id: "cloud-databases",
    label: "Cloud Databases",
    tip: "Open databases stored in the ChessBase Cloud",
    icon: Cloud,
    views: both,
  },
  {
    id: "db-copy",
    label: "Copy",
    tip: "Copy selected games",
    icon: Copy,
    views: db,
  },
  {
    id: "db-paste",
    label: "Paste",
    tip: "Paste games into this database",
    icon: ClipboardPaste,
    views: db,
  },
  {
    id: "open-board",
    label: "Board",
    tip: "Open the selected game in its board tab",
    icon: LayoutGrid,
    keys: "Enter",
    views: db,
  },
  {
    id: "filter-games",
    label: "Filter Games",
    tip: "Filter the games shown in this list",
    icon: Filter,
    keys: "Ctrl+F",
    views: db,
  },
  {
    id: "search-online",
    label: "Online",
    tip: "Search games in the online database",
    icon: Globe,
    views: db,
  },
  {
    id: "search-hard-disk",
    label: "Hard Disk",
    tip: "Search games in databases on your hard disk",
    icon: HardDrive,
    views: db,
  },
  {
    id: "prepare-against",
    label: "Prepare Against",
    tip: "Prepare against a player",
    icon: Swords,
    views: db,
  },
  {
    id: "player-encyclopedia",
    label: "Player Encyclopedia",
    tip: "Open the player encyclopedia",
    icon: Users,
    views: db,
  },
  {
    id: "opening-reference",
    label: "Opening reference",
    tip: "Search the reference database for this opening",
    icon: BookOpen,
    views: db,
  },
  {
    id: "generate-repertoire",
    label: "Generate Repertoire",
    tip: "Create a repertoire database",
    icon: ListTree,
    views: db,
  },
  {
    id: "backup-database",
    label: "Backup Database",
    tip: "Archive the database",
    icon: Archive,
    views: db,
  },
  {
    id: "database-properties",
    label: "Properties",
    tip: "Show database properties",
    icon: Info,
    views: db,
  },
  {
    id: "check-integrity",
    label: "Check Integrity",
    tip: "Check the database for errors",
    icon: Wrench,
    views: db,
  },
  {
    id: "find-double-games",
    label: "Find Double Games",
    tip: "Mark duplicate games",
    icon: Copy,
    views: db,
  },
  {
    id: "sort-database",
    label: "Sort Database",
    tip: "Sort the database physically",
    icon: ArrowDownUp,
    views: db,
  },
  {
    id: "previous-page",
    label: "Previous page",
    tip: "Show the previous 200 entries (workbench)",
    icon: ChevronsLeft,
    keys: "PageUp",
    views: db,
  },
  {
    id: "next-page",
    label: "Next page",
    tip: "Show the next 200 entries (workbench)",
    icon: ChevronsRight,
    keys: "PageDown",
    views: db,
  },

  // Game window
  {
    id: "paste-game",
    label: "Paste Game",
    tip: "Paste game (ASCII) from clipboard",
    icon: ClipboardPaste,
    keys: "Ctrl+V",
    views: game,
  },
  {
    id: "paste-position",
    label: "Paste Position",
    tip: "Paste a position (ASCII or FEN) from clipboard",
    icon: ClipboardPaste,
    views: game,
  },
  {
    id: "copy-game",
    label: "Copy Game",
    tip: "Copy game notation to clipboard",
    icon: Copy,
    keys: "Ctrl+C",
    views: game,
  },
  {
    id: "copy-position",
    label: "Copy Position",
    tip: "Copy current position (FEN) to clipboard",
    icon: Copy,
    views: game,
  },
  {
    id: "new-game",
    label: "New Game",
    tip: "Start a new game",
    icon: FilePlus,
    keys: "Ctrl+N",
    views: both,
  },
  {
    id: "input-mode",
    label: "Input mode",
    tip: "Enter moves by keyboard",
    icon: Keyboard,
    views: game,
  },
  {
    id: "undo",
    label: "Undo",
    tip: "Undo last change to game",
    icon: Undo2,
    keys: "Ctrl+Z",
    views: game,
  },
  {
    id: "redo",
    label: "Redo",
    tip: "Redo last change to game",
    icon: Redo2,
    views: game,
  },
  {
    id: "save-pgn",
    label: "Save PGN",
    tip: "Save this game to a PGN file (ChessBase: Save)",
    icon: Save,
    keys: "Ctrl+S",
    views: game,
  },
  {
    id: "close-game",
    label: "Close game",
    tip: "Close this game",
    icon: X,
    views: game,
  },
  {
    id: "find-position-online",
    label: "Online",
    tip: "Find the current position in the online database",
    icon: Globe,
    views: game,
  },
  {
    id: "find-position-db",
    label: "Hard Disk",
    tip: "Find the current position in a database",
    icon: HardDrive,
    views: game,
  },
  {
    id: "add-kibitzer",
    label: "Add Kibitzer",
    tip: "Add a Kibitz engine pane to analyse the game",
    icon: Cpu,
    keys: "Ctrl+K",
    views: game,
  },
  {
    id: "remove-kibitzer",
    label: "Remove Kibitzer",
    tip: "Remove the Kibitz engine pane",
    icon: X,
    keys: "Ctrl+Shift+K",
    views: game,
  },
  {
    id: "toggle-analysis",
    label: "Infinite Analysis",
    tip: "Start or stop analysis of the current position",
    icon: Play,
    keys: "Alt+F2",
    views: game,
  },
  {
    id: "engine-management",
    label: "Engine Management",
    tip: "Manage installed engines",
    icon: Settings,
    views: game,
  },
  {
    id: "edit-game-data",
    label: "Edit Game Data",
    tip: "Set player names, tournament, result, etc. for this game",
    icon: TableProperties,
    keys: "F2",
    views: game,
  },
  {
    id: "load-previous-game",
    label: "Load Previous Game",
    tip: "Load previous game from database",
    icon: ChevronsLeft,
    keys: "Ctrl+F11",
    views: game,
  },
  {
    id: "load-next-game",
    label: "Load Next Game",
    tip: "Load next game from database",
    icon: ChevronsRight,
    keys: "F11",
    views: game,
  },
  {
    id: "text-after",
    label: "Text After Move",
    tip: "Annotate with text after move",
    icon: MessageSquareText,
    keys: "Ctrl+A",
    views: game,
  },
  {
    id: "text-before",
    label: "Text Before Move",
    tip: "Annotate with text before move",
    icon: MessageSquareQuote,
    keys: "Ctrl+Shift+A",
    views: game,
  },
  {
    id: "begin-variation",
    label: "Begin Variation",
    tip: "Start a variation at this move",
    icon: Split,
    views: game,
  },
  {
    id: "delete-variation",
    label: "Delete Variation",
    tip: "Delete the current variation",
    icon: Trash2,
    keys: "Ctrl+Y",
    views: game,
  },
  {
    id: "promote-variation",
    label: "Promote Variation",
    tip: "Promote the current variation to main line of previous level",
    icon: ArrowUpToLine,
    keys: "Alt+Up",
    views: game,
  },
  {
    id: "flip-board",
    label: "Flip board",
    tip: "Rotate the chess board",
    icon: RotateCcw,
    keys: "Ctrl+F",
    views: game,
  },
  {
    id: "board-design",
    label: "Board Design",
    tip: "Change board design and style",
    icon: Palette,
    views: game,
  },
  {
    id: "opening-report",
    label: "Opening Report",
    tip: "Create an opening report for this position",
    icon: ChartColumn,
    views: game,
  },
  {
    id: "similar-games",
    label: "Similar Moves",
    tip: "Find games with similar moves",
    icon: Search,
    views: game,
  },
  {
    id: "deep-analysis",
    label: "Deep Analysis",
    tip: "Analyse the position deeply with the engine",
    icon: Microscope,
    views: game,
  },
  {
    id: "tactical-analysis",
    label: "Tactical Analysis",
    tip: "Check the game for tactical mistakes",
    icon: Gauge,
    views: game,
  },
  {
    id: "training-annotation",
    label: "Training Annotation",
    tip: "Insert a training question",
    icon: GraduationCap,
    views: game,
  },
  {
    id: "hints",
    label: "Hints",
    tip: "Show hints for the current position",
    icon: Lightbulb,
    views: game,
  },

  // Both windows
  {
    id: "layout-standard",
    label: "Standard Layout",
    tip: "Reset panes to the standard layout",
    icon: PanelsTopLeft,
    keys: "Ctrl+Alt+Shift+F9",
    views: both,
  },
  {
    id: "layout-board-only",
    label: "Big Board",
    tip: "Give the board most of the window",
    icon: Maximize2,
    views: game,
  },
  {
    id: "layout-wide-list",
    label: "Wide List",
    tip: "Give the game list most of the window",
    icon: Columns2,
    views: db,
  },
  {
    id: "layout-tall-engine",
    label: "Big Engine",
    tip: "Give the engine pane more room",
    icon: Rows2,
    views: game,
  },
  {
    id: "theme-white",
    label: "White Theme",
    tip: "Use the White theme",
    icon: Palette,
    views: both,
  },
  {
    id: "theme-blue",
    label: "Blue Theme",
    tip: "Use the Blue theme",
    icon: Palette,
    views: both,
  },
  {
    id: "theme-gray",
    label: "Gray Theme",
    tip: "Use the Gray theme",
    icon: Palette,
    views: both,
  },
  {
    id: "theme-black",
    label: "Black Theme",
    tip: "Use the Black theme",
    icon: Palette,
    views: both,
  },
  {
    id: "command-search",
    label: "Search commands",
    tip: "Find any command by name (workbench)",
    icon: Search,
    keys: "Ctrl+Shift+Space",
    views: both,
  },
  {
    id: "options",
    label: "Options",
    tip: "Set the main program options",
    icon: Settings,
    keys: "Ctrl+Alt+O",
    views: both,
  },
  {
    id: "help",
    label: "Help",
    tip: "Open the help",
    icon: LifeBuoy,
    keys: "F1",
    views: both,
  },
  {
    id: "feedback",
    label: "Support contact",
    tip: "Contact support",
    icon: Mail,
    views: both,
  },
  {
    id: "decrease-lines",
    label: "Decrease number of lines",
    tip: "Decrease the number of main lines computed by the engine",
    icon: Minus,
    keys: "-",
    views: game,
  },
  {
    id: "increase-lines",
    label: "Increase number of lines",
    tip: "Increase the number of main lines computed by the engine",
    icon: Plus,
    keys: "+",
    views: game,
  },
  {
    id: "stop-process",
    label: "Stop Process",
    tip: "Stop a running process",
    icon: Square,
    views: both,
  },
  {
    id: "database-window",
    label: "Database",
    tip: "Show the database list",
    icon: Database,
    views: game,
  },
];

export const commands: Record<string, CommandMeta> = Object.fromEntries(
  list.map((c) => [c.id, c]),
);
export const commandList = list;

export interface RibbonGroup {
  label: string;
  items: { id: string; size?: "large" | "small" }[];
}
export interface RibbonTab {
  label: string;
  groups: RibbonGroup[];
}

const L = (id: string) => ({ id, size: "large" as const });
const S = (id: string) => ({ id, size: "small" as const });

const viewTab: RibbonTab = {
  label: "View",
  groups: [
    { label: "Layout", items: [L("layout-standard")] },
    {
      label: "Theme",
      items: [
        S("theme-white"),
        S("theme-blue"),
        S("theme-gray"),
        S("theme-black"),
      ],
    },
  ],
};
const helpTab: RibbonTab = {
  label: "Help",
  groups: [{ label: "Help website", items: [L("help"), L("feedback")] }],
};

/** Tabs and groups follow ChessBase 18's database and game window ribbons. */
export const ribbons: Record<ViewId, RibbonTab[]> = {
  database: [
    {
      label: "Home",
      groups: [
        { label: "Copy", items: [L("db-paste"), S("db-copy")] },
        {
          label: "Database",
          items: [
            L("open-database"),
            L("open-pgn"),
            S("new-database"),
            S("my-databases"),
            S("cloud-databases"),
          ],
        },
        { label: "Board", items: [L("open-board")] },
        {
          label: "Search games",
          items: [L("filter-games"), S("search-online"), S("search-hard-disk")],
        },
        {
          label: "Players",
          items: [L("prepare-against"), S("player-encyclopedia")],
        },
      ],
    },
    {
      label: "Report",
      groups: [
        {
          label: "Report",
          items: [L("opening-reference"), L("generate-repertoire")],
        },
      ],
    },
    {
      label: "Maintenance",
      groups: [
        {
          label: "Maintenance",
          items: [L("backup-database"), S("database-properties")],
        },
        {
          label: "Improve",
          items: [
            S("check-integrity"),
            S("find-double-games"),
            S("sort-database"),
          ],
        },
      ],
    },
    {
      ...viewTab,
      groups: [
        {
          label: "Layout",
          items: [L("layout-standard"), L("layout-wide-list")],
        },
        viewTab.groups[1],
      ],
    },
    helpTab,
  ],
  game: [
    {
      label: "Home",
      groups: [
        {
          label: "File",
          items: [
            L("save-pgn"),
            S("open-database"),
            S("open-pgn"),
            S("close-game"),
          ],
        },
        {
          label: "Clipboard",
          items: [
            L("paste-game"),
            S("paste-position"),
            S("copy-game"),
            S("copy-position"),
          ],
        },
        {
          label: "Game",
          items: [S("new-game"), S("input-mode"), S("undo"), S("redo")],
        },
        {
          label: "Find Position",
          items: [S("find-position-online"), S("find-position-db")],
        },
        {
          label: "Engine",
          items: [
            L("add-kibitzer"),
            L("toggle-analysis"),
            S("remove-kibitzer"),
            S("engine-management"),
          ],
        },
        {
          label: "Database",
          items: [
            L("database-window"),
            S("edit-game-data"),
            S("load-previous-game"),
            S("load-next-game"),
          ],
        },
      ],
    },
    {
      label: "Insert",
      groups: [
        { label: "Moves", items: [L("text-after"), S("text-before")] },
        {
          label: "Variations",
          items: [
            S("begin-variation"),
            S("delete-variation"),
            S("promote-variation"),
          ],
        },
      ],
    },
    {
      label: "Board",
      groups: [{ label: "Board", items: [L("flip-board"), L("board-design")] }],
    },
    {
      label: "Report",
      groups: [
        { label: "Report", items: [L("opening-report"), L("similar-games")] },
      ],
    },
    {
      label: "Analysis",
      groups: [
        { label: "Deep Analysis", items: [L("deep-analysis")] },
        { label: "Tactical Analysis", items: [L("tactical-analysis")] },
        {
          label: "Engine lines",
          items: [S("increase-lines"), S("decrease-lines")],
        },
      ],
    },
    {
      label: "Training",
      groups: [
        { label: "Training", items: [L("training-annotation"), L("hints")] },
      ],
    },
    {
      ...viewTab,
      groups: [
        {
          label: "Layout",
          items: [
            L("layout-standard"),
            L("layout-board-only"),
            L("layout-tall-engine"),
          ],
        },
        viewTab.groups[1],
      ],
    },
    helpTab,
  ],
};
