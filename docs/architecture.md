Oui. Pour PortSpy, je ferais une architecture propre dès le départ, mais sans tomber dans le piège du monorepo avec 18 crates inutiles.

Le bon compromis ici : un workspace Rust avec quelques crates bien séparées, chacune avec une responsabilité nette.

```text
portspy/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── rustfmt.toml
├── clippy.toml
├── deny.toml
├── .editorconfig
├── .gitignore
├── LICENSE
├── README.md
├── CONTRIBUTING.md
├── SECURITY.md
│
├── crates/
│   ├── portspy-cli/
│   │   └── src/
│   │       ├── main.rs
│   │       ├── app.rs
│   │       ├── args.rs
│   │       └── commands/
│   │           ├── mod.rs
│   │           ├── tui.rs
│   │           ├── list.rs
│   │           ├── inspect.rs
│   │           └── kill.rs
│   │
│   ├── portspy-core/
│   │   └── src/
│   │       ├── lib.rs
│   │       │
│   │       ├── domain/
│   │       │   ├── mod.rs
│   │       │   ├── listener.rs
│   │       │   ├── process.rs
│   │       │   ├── origin.rs
│   │       │   ├── application.rs
│   │       │   └── protocol.rs
│   │       │
│   │       ├── ports/
│   │       │   ├── mod.rs
│   │       │   └── scanner.rs
│   │       │
│   │       ├── processes/
│   │       │   ├── mod.rs
│   │       │   ├── inspector.rs
│   │       │   ├── ancestry.rs
│   │       │   └── tree.rs
│   │       │
│   │       ├── detection/
│   │       │   ├── mod.rs
│   │       │   ├── origin.rs
│   │       │   ├── application.rs
│   │       │   └── rules/
│   │       │       ├── mod.rs
│   │       │       ├── agents.rs
│   │       │       ├── terminals.rs
│   │       │       ├── runtimes.rs
│   │       │       └── frameworks.rs
│   │       │
│   │       ├── snapshot/
│   │       │   ├── mod.rs
│   │       │   ├── builder.rs
│   │       │   └── diff.rs
│   │       │
│   │       ├── actions/
│   │       │   ├── mod.rs
│   │       │   └── kill.rs
│   │       │
│   │       └── error.rs
│   │
│   ├── portspy-platform/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── traits.rs
│   │       └── windows/
│   │           ├── mod.rs
│   │           ├── ports.rs
│   │           ├── processes.rs
│   │           ├── metrics.rs
│   │           ├── handles.rs
│   │           └── kill.rs
│   │
│   └── portspy-tui/
│       └── src/
│           ├── lib.rs
│           ├── app.rs
│           ├── event.rs
│           ├── state.rs
│           │
│           ├── ui/
│           │   ├── mod.rs
│           │   ├── layout.rs
│           │   ├── theme.rs
│           │   ├── responsive.rs
│           │   │
│           │   ├── screens/
│           │   │   ├── mod.rs
│           │   │   ├── listeners.rs
│           │   │   └── details.rs
│           │   │
│           │   └── components/
│           │       ├── mod.rs
│           │       ├── header.rs
│           │       ├── listeners_table.rs
│           │       ├── details_panel.rs
│           │       ├── process_tree.rs
│           │       ├── footer.rs
│           │       ├── search.rs
│           │       └── confirm.rs
│           │
│           ├── input/
│           │   ├── mod.rs
│           │   ├── keymap.rs
│           │   └── actions.rs
│           │
│           └── format/
│               ├── mod.rs
│               ├── memory.rs
│               ├── duration.rs
│               └── address.rs
│
├── tests/
│   ├── integration/
│   │   ├── listener_discovery.rs
│   │   ├── ancestry_detection.rs
│   │   ├── origin_detection.rs
│   │   └── kill_process.rs
│   │
│   └── fixtures/
│       ├── process_trees/
│       └── commands/
│
├── docs/
│   ├── architecture/
│   │   ├── overview.md
│   │   ├── data-flow.md
│   │   ├── process-model.md
│   │   └── tui.md
│   │
│   ├── adr/
│   │   ├── 0001-rust-workspace.md
│   │   ├── 0002-platform-boundary.md
│   │   ├── 0003-process-origin-detection.md
│   │   └── 0004-tui-state-model.md
│   │
│   └── development/
│       ├── setup.md
│       ├── testing.md
│       └── release.md
│
├── scripts/
│   ├── check.ps1
│   ├── test.ps1
│   └── release.ps1
│
└── .github/
    ├── workflows/
    │   ├── ci.yml
    │   └── release.yml
    └── ISSUE_TEMPLATE/
```

La séparation la plus importante est celle-ci :

```text
portspy-core
     │
     ├── logique métier
     ├── détection
     ├── modèles
     └── aucune API Windows directe

portspy-platform
     │
     └── Win32 / OS

portspy-tui
     │
     └── Ratatui uniquement

portspy-cli
     │
     └── composition + arguments + lancement
```

Autrement dit, le TUI n’a jamais le droit de faire ça directement :

```rust
GetExtendedTcpTable(...)
```

Il demande au core :

```rust
let snapshot = scanner.snapshot().await?;
```

Et le core utilise derrière une abstraction plateforme.

C’est ce qui évite qu’après 3 semaines tu te retrouves avec du Win32 mélangé dans `ui.rs`.

Le flux principal serait :

```text
Win32
  │
  ▼
portspy-platform
  │
  ├─ listening ports
  ├─ PID
  ├─ process metadata
  └─ ancestry
  │
  ▼
portspy-core
  │
  ├─ normalise
  ├─ enrichit
  ├─ détecte application
  ├─ détecte origine
  └─ produit Snapshot
  │
  ▼
portspy-tui
  │
  └─ affiche uniquement
```

Le type central doit être un snapshot immuable, par exemple :

```rust
pub struct SystemSnapshot {
    pub captured_at: Instant,
    pub listeners: Vec<Listener>,
}
```

Et chaque listener :

```rust
pub struct Listener {
    pub id: ListenerId,

    pub protocol: Protocol,
    pub address: IpAddr,
    pub port: u16,

    pub process: ProcessInfo,
    pub application: ApplicationInfo,
    pub origin: OriginInfo,
}
```

Pour `ProcessInfo`, je garderais des champs factuels uniquement :

```rust
pub struct ProcessInfo {
    pub pid: ProcessId,
    pub parent_pid: Option<ProcessId>,

    pub name: String,
    pub executable: Option<PathBuf>,
    pub command_line: Option<String>,
    pub cwd: Option<PathBuf>,

    pub started_at: Option<SystemTime>,

    pub memory_bytes: Option<u64>,
    pub cpu_percent: Option<f32>,

    pub ancestors: Vec<ProcessAncestor>,
}
```

Et surtout ne mélange pas les faits avec les inférences.

Ça :

```rust
process.name == "node.exe"
```

est un fait.

Ça :

```rust
application == NextJs
```

est une inférence.

Ça :

```rust
origin == Codex
```

est une autre inférence.

Donc :

```rust
pub struct ApplicationInfo {
    pub kind: ApplicationKind,
    pub label: String,
    pub confidence: Confidence,
}

pub struct OriginInfo {
    pub kind: OriginKind,
    pub label: String,
    pub confidence: Confidence,
    pub matched_pid: Option<ProcessId>,
}
```

Très important pour garder le moteur fiable.

Pour la détection des origines, pas de gigantesque :

```rust
if process.contains("codex") {
...
} else if ...
```

dans un fichier de 900 lignes.

Je ferais des règles déclaratives.

Conceptuellement :

```rust
OriginRule {
    origin: OriginKind::Codex,
    executable_names: &[
        "codex.exe",
        "codex",
    ],
}
```

Et pareil pour Claude :

```text
detection/
└── rules/
    ├── agents.rs
    ├── terminals.rs
    ├── frameworks.rs
    └── runtimes.rs
```

Tu pourras facilement ajouter plus tard :

```text
Gemini CLI
OpenCode
Aider
Continue
Cline
Windsurf
```

sans massacrer le moteur.

La détection d’application suit exactement le même principe.

On commence par le plus spécifique.

Par exemple :

```text
node.exe

command:
next dev

→ Next.js
```

Puis :

```text
node.exe

command:
vite

→ Vite
```

Et si rien n’est reconnu :

```text
→ Node.js
```

Donc :

```text
specific framework
        ↓
runtime
        ↓
raw executable
```

La règle doit être :

> PortSpy préfère une information précise quand elle est prouvée, sinon il retombe proprement vers une information plus générale.

Pour l’OS, je créerais une frontière claire dès maintenant.

Quelque chose comme :

```rust
pub trait Platform {
    fn listeners(&self) -> Result<Vec<RawListener>>;
    fn process(&self, pid: ProcessId) -> Result<Option<RawProcess>>;
    fn kill(&self, pid: ProcessId) -> Result<KillResult>;
}
```

Mais je n’essaierais pas de créer 25 traits abstraits “pour Linux un jour”.

Seulement l’abstraction nécessaire.

Windows derrière :

```text
portspy-platform/src/windows/
```

La récupération de ports peut utiliser :

```text
GetExtendedTcpTable
GetExtendedUdpTable
```

La récupération de process :

Win32 native APIs en priorité.

Pour certaines métadonnées plus difficiles, on peut utiliser d’autres interfaces Windows, mais WMI ne devrait pas devenir le chemin chaud du refresh toutes les secondes.

C’est important niveau perf.

Le refresh devrait plutôt ressembler à :

```text
t = 0

collect ports
       ↓
obtenir liste unique PID
       ↓
collect metadata pour chaque PID
       ↓
reconstruire ancestry
       ↓
enrichir
       ↓
snapshot
```

Et surtout pas :

```text
pour chaque port
    query WMI
    query WMI
    query WMI
```

sinon le TUI sera lent.

Je mettrais aussi un cache de métadonnées process.

Par exemple :

```text
PID 18432
start_time 23:02:41
```

peut servir comme identité.

Car les PID sont recyclables sous Windows.

Donc la clé correcte n’est pas juste :

```text
PID
```

mais conceptuellement :

```text
PID + process start time
```

Ça évite un bug subtil où un PID disparaît puis est réutilisé par un autre programme entre deux snapshots.

Pour le TUI, je séparerais strictement l’état des widgets.

Pas :

```text
listeners_table.rs
```

qui décide lui-même ce qui est sélectionné.

Le state appartient à l’application :

```rust
pub struct AppState {
    pub snapshot: SystemSnapshot,
    pub selected: Option<ListenerId>,
    pub filter: FilterState,
    pub sort: SortState,
    pub mode: AppMode,
}
```

Puis les composants sont presque purs :

```rust
render_listeners(frame, area, &state)
```

Pas de logique métier dans le rendering.

`AppMode` :

```rust
enum AppMode {
    Browse,
    Inspect,
    Search,
    ConfirmKill,
}
```

Simple et suffisant.

Pour la sélection, je ne stockerais surtout pas :

```rust
selected_index: usize
```

comme identité principale.

Parce qu’au refresh :

```text
:3001 disparaît
```

et tous les indexes bougent.

Utilise :

```rust
selected: Option<ListenerId>
```

et seulement à l’affichage on retrouve sa position.

Pour l’ID d’un listener, quelque chose de stable pendant sa vie :

```text
protocol
local address
port
pid
process start time
```

ou un wrapper/hash construit dessus.

Le TUI aura donc une hiérarchie très claire :

```text
App
├── Event loop
├── State
├── Commands/actions
└── UI
    ├── Screens
    └── Components
```

Le cycle :

```text
keyboard
   │
   ▼
InputAction
   │
   ▼
App::update()
   │
   ├─ update state
   ├─ request kill
   └─ request refresh
   │
   ▼
render(state)
```

C’est beaucoup plus maintenable qu’un gigantesque `match KeyCode`.

Je définirais même les actions séparément :

```rust
enum Action {
    Quit,
    MoveUp,
    MoveDown,
    Inspect,
    Back,
    Search,
    Refresh,
    RequestKill,
    ConfirmKill,
    Cancel,
}
```

Puis :

```text
keymap.rs
```

transforme :

```text
j
↓
```

en :

```text
MoveDown
```

Du coup plus tard tu peux permettre :

```text
Vim keys
Arrow keys
Custom keybindings
```

sans changer toute l’application.

Pour le threading, V0 reste également simple.

Je ferais :

```text
UI thread
    │
    ├── input events
    └── render
           ▲
           │
      channel snapshot
           │
background collector
```

Le scanner tourne hors du render thread.

Jamais :

```text
render()
  └── GetExtendedTcpTable()
```

Sinon le terminal freeze si Windows tarde.

Le background collector produit :

```rust
CollectorEvent::Snapshot(snapshot)
```

ou :

```rust
CollectorEvent::Error(error)
```

Le TUI reste responsive.

Pour tuer un process :

```text
user K
 ↓
state = ConfirmKill
 ↓
Y
 ↓
action service
 ↓
Platform::kill(pid)
 ↓
force refresh
```

Et les erreurs ne crashent jamais le programme.

Exemple :

```text
Access denied
```

devient une notification dans le footer :

```text
Could not stop PID 712 — access denied.
```

Pas un `unwrap()`.

D’ailleurs je mettrais une règle très stricte dans le repo :

```text
No unwrap() / expect() in production paths
```

Sauf impossibilité structurelle documentée.

Pour les erreurs :

```rust
pub enum PortSpyError {
    Platform(PlatformError),
    PermissionDenied { pid: u32 },
    ProcessGone { pid: u32 },
    Unsupported(...),
}
```

Pas besoin de créer 400 variantes non plus.

Pour les tests, trois couches.

Unitaires pour les inférences :

```text
node + next dev
→ Next.js

ancestry contains codex
→ Codex

ancestry contains claude
→ Claude Code
```

Tests avec fixtures pour les arbres :

```text
WindowsTerminal
└── pwsh
    └── codex
        └── node
```

Et intégration Windows pour la vraie découverte.

Par exemple, le test peut lancer un petit TCP listener contrôlé :

```text
bind localhost:0
```

récupérer le port dynamique, puis vérifier que PortSpy le trouve et l’associe au bon PID.

Ça évite les tests fragiles qui supposent que `:3000` est libre.

Je garderais également un dossier `docs/adr/` dès maintenant. Pas pour écrire un roman sur chaque `enum`, mais pour les décisions qui pourraient coûter cher à inverser.

Les quatre premières ADR que je vois :

```text
ADR-001
Workspace layout and crate boundaries

ADR-002
Windows-native process and listener discovery

ADR-003
Origin/application inference model

ADR-004
Snapshot-driven TUI architecture
```

Et une règle : pas de `utils.rs`.

Très souvent `utils.rs` devient la poubelle du repo.

Si une fonction formate une durée :

```text
format/duration.rs
```

Si elle travaille sur un process :

```text
processes/
```

Si on ne sait pas où la mettre, c’est souvent que sa responsabilité n’est pas claire.

Même chose pour :

```text
helpers.rs
common.rs
misc.rs
manager.rs
service.rs
```

Ces noms doivent être évités sauf lorsqu’ils désignent réellement un concept métier précis.

À la racine, le `Cargo.toml` :

```toml
[workspace]
resolver = "2"

members = [
    "crates/portspy-cli",
    "crates/portspy-core",
    "crates/portspy-platform",
    "crates/portspy-tui",
]
```

Dépendances centralisées :

```toml
[workspace.dependencies]
anyhow = "..."
thiserror = "..."
tracing = "..."
ratatui = "..."
crossterm = "..."
windows = "..."
```

Puis chaque crate utilise :

```toml
ratatui.workspace = true
```

Ça évite les versions divergentes.

Je mettrais également des lints workspace assez agressifs :

```toml
[workspace.lints.rust]
unsafe_code = "deny"

[workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
dbg_macro = "deny"
todo = "deny"
```

Avec une nuance : l’API Win32 nécessitera probablement du `unsafe`.

Donc idéalement :

```text
unsafe autorisé uniquement dans portspy-platform
```

Et interdit partout ailleurs.

Ça c’est une excellente frontière.

Le core devient du Rust safe.

Le TUI devient du Rust safe.

Seule la couche plateforme touche à l’unsafe Windows.

Pour le logging, même avec un TUI :

```text
tracing
tracing-subscriber
```

avec possibilité :

```bash
PORTSPY_LOG=debug portspy
```

et éventuellement fichier :

```text
%LOCALAPPDATA%\PortSpy\portspy.log
```

mais ça peut arriver après.

La version V0 devrait donc avoir exactement quatre blocs :

```text
portspy-cli
Entrypoint et commandes

portspy-core
Cerveau du produit

portspy-platform
Interaction Windows

portspy-tui
Expérience utilisateur
```

Je n’ajouterais aucun autre crate tant qu’on n’a pas une vraie pression architecturale.

Et la dépendance doit rester unidirectionnelle :

```text
             ┌──────────────┐
             │ portspy-cli  │
             └──────┬───────┘
                    │
            ┌───────┴────────┐
            ▼                ▼
     portspy-tui        portspy-core
            │                ▲
            └──────┬─────────┘
                   │
                   ▼
           portspy-platform
```

Plus précisément, je préférerais même que `core` dépende d’un trait de plateforme et que l’implémentation soit injectée depuis le CLI, pour éviter que le domaine dépende de Windows. Mais inutile de créer une architecture hexagonale caricaturale : une boundary claire suffit.

Objectif : dans six mois, quand on ajoutera Linux/WSL ou un mode agent, on ne doit pas avoir besoin de réécrire le TUI ni le moteur de détection.

Le repo doit rester assez petit pour qu’un humain puisse comprendre sa structure en 30 secondes, mais assez découpé pour qu’un agent ne transforme pas `main.rs` en fichier de 4 000 lignes. C’est exactement l’équilibre que je viserais pour PortSpy.
