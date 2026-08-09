
L’objectif de `v0.1.0` reste strict :

> lancer `portspy`, voir les ports en écoute sur Windows, comprendre quel programme les utilise et d’où il vient, inspecter sa chaîne de processus et pouvoir le fermer depuis un TUI extrêmement propre.

## 1. Roadmap globale

| Phase | Objectif         | Livrable                       |
| ----- | ---------------- | ------------------------------ |
| 0     | Fondation repo   | workspace propre + CI          |
| 1     | Modèle métier    | types et contrats stables      |
| 2     | Ports Windows    | listeners TCP fiables          |
| 3     | Process Windows  | PID + metadata                 |
| 4     | Process ancestry | arbre parental                 |
| 5     | Detection engine | Codex / Claude / Cursor / apps |
| 6     | Snapshot engine  | collecte atomique + refresh    |
| 7     | TUI foundation   | interface navigable            |
| 8     | TUI premium      | inspect/search/sort/responsive |
| 9     | Kill process     | fermeture sécurisée            |
| 10    | CLI              | list / inspect / kill / JSON   |
| 11    | Hardening        | race conditions + permissions  |
| 12    | Release          | `v0.1.0`                       |

Je ne touche pas encore au daemon, au MCP, au supervisor, aux doublons automatiques ou à Docker management.

---

# Phase 0 — Fondation du repo

Avant la moindre feature.

Structure :

```text
portspy/
├── crates/
│   ├── portspy-cli/
│   ├── portspy-core/
│   ├── portspy-platform/
│   └── portspy-tui/
│
├── tests/
├── docs/
├── scripts/
└── .github/
```

Configurer immédiatement :

```text
Cargo workspace
rust-toolchain.toml
rustfmt.toml
clippy
cargo-deny
.editorconfig
GitHub Actions
Dependabot/Renovate éventuellement
```

Quality gates :

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Dans le core et le TUI :

```text
unsafe interdit
unwrap interdit
expect interdit
todo! interdit
dbg! interdit
```

`unsafe` accepté uniquement dans les modules Windows qui en ont réellement besoin.

Documentation initiale :

```text
docs/
├── architecture/
│   ├── overview.md
│   ├── data-flow.md
│   └── crates.md
└── adr/
    ├── ADR-001-workspace-boundaries.md
    ├── ADR-002-windows-native-discovery.md
    ├── ADR-003-detection-engine.md
    └── ADR-004-snapshot-tui.md
```

Gate :

```text
cargo check --workspace
cargo test --workspace
```

doivent être verts avant Phase 1.

---

# Phase 1 — Modèle métier

Aucune API Windows pour l’instant.

Créer les types fondamentaux.

```rust
Listener
ListenerId
Protocol
SocketAddress
ProcessId
ProcessIdentity
ProcessInfo
ProcessAncestor
ApplicationInfo
OriginInfo
Confidence
SystemSnapshot
```

Distinction essentielle :

```text
Raw data
→ ce que Windows affirme

Inference
→ ce que PortSpy déduit
```

Par exemple :

```text
Fact:
process = node.exe

Inference:
application = Next.js

Inference:
origin = Codex
```

Ça ne doit jamais être mélangé.

`ProcessIdentity` devrait être basé conceptuellement sur :

```text
PID + process creation time
```

et non uniquement sur le PID.

Sinon Windows peut recycler un PID.

Définir également :

```rust
enum OriginKind {
    Codex,
    ClaudeCode,
    Cursor,
    VsCode,
    WindowsTerminal,
    PowerShell,
    Cmd,
    Wsl,
    Docker,
    Unknown,
}
```

et :

```rust
enum ApplicationKind {
    NextJs,
    Vite,
    Node,
    Python,
    Rust,
    PostgreSql,
    Redis,
    Docker,
    Unknown,
}
```

Pas de stringly-typed architecture partout.

Gate :

tests unitaires sur tous les invariants des modèles.

---

# Phase 2 — Découverte des ports Windows

Premier morceau réellement utile.

Implémentation dans :

```text
portspy-platform/src/windows/network/
├── mod.rs
├── tcp.rs
└── conversion.rs
```

Commencer uniquement avec TCP listeners.

API Windows :

```text
GetExtendedTcpTable
```

Récupérer :

```text
IPv4
IPv6
local address
local port
state
PID
```

Ne pas intégrer UDP immédiatement.

PortSpy doit distinguer :

```text
127.0.0.1
::1
0.0.0.0
::
specific LAN address
```

Avec une abstraction comme :

```rust
enum Exposure {
    Loopback,
    Interface,
    AllInterfaces,
}
```

Premier executable temporaire :

```bash
portspy list
```

Même si le TUI n’existe pas encore.

Sortie temporaire :

```text
TCP  127.0.0.1:3000  PID 18432
TCP  0.0.0.0:5432    PID 8820
```

Tests d’intégration :

le test ouvre lui-même un listener sur :

```rust
TcpListener::bind("127.0.0.1:0")
```

puis vérifie que PortSpy le découvre.

Pas de port hardcodé.

Gate :

PortSpy retrouve de manière répétable un listener créé par son propre test.

---

# Phase 3 — Inspection des processus

Structure :

```text
windows/process/
├── mod.rs
├── snapshot.rs
├── metadata.rs
├── command_line.rs
├── path.rs
└── metrics.rs
```

Pour chaque PID unique rencontré :

récupérer :

```text
process name
executable path
parent PID
creation time
memory
command line
```

Le `cwd` peut être plus compliqué sous Windows.

Donc :

```text
cwd: Option<PathBuf>
```

et jamais :

```text
cwd: PathBuf
```

en supposant qu’il sera toujours disponible.

Même logique pour command line/path.

Process inaccessible :

```text
AccessDenied
```

n’est pas une erreur fatale.

PortSpy affiche ce qu’il connaît.

Important pour les performances : ne jamais faire une requête coûteuse par colonne et par listener.

Faire :

```text
listeners
   ↓
unique PIDs
   ↓
batch process inspection
   ↓
join
```

Ajouter un petit cache :

```text
ProcessIdentity → ProcessInfo
```

Gate :

pour un listener de test, PortSpy donne le bon :

```text
PID
process name
parent PID
creation time
```

---

# Phase 4 — Reconstruction des ancêtres

Ici on arrive au cœur de PortSpy.

À partir du process écoutant :

```text
node.exe
```

remonter :

```text
node.exe
↑
codex.exe
↑
pwsh.exe
↑
WindowsTerminal.exe
```

Maximum depth explicite :

```text
32
```

ou équivalent.

Protection contre :

```text
cycle impossible/corruption
parent disparu
permission denied
PID recyclé
```

Modèle :

```rust
ProcessAncestry {
    leaf: ProcessIdentity,
    ancestors: Vec<ProcessAncestor>,
}
```

Tests par fixtures.

Exemple fixture :

```text
PID 10 WindowsTerminal
PID 11 pwsh parent=10
PID 12 codex parent=11
PID 13 node parent=12
```

Résultat :

```text
node
└── codex
    └── pwsh
        └── WindowsTerminal
```

Ou l’orientation inverse selon le modèle interne.

Gate :

reconstruction déterministe et testée sans dépendre du PC.

---

# Phase 5 — Detection engine

Maintenant PortSpy transforme des données brutes en information utile.

Structure :

```text
core/src/detection/
├── mod.rs
├── origin/
│   ├── mod.rs
│   ├── detector.rs
│   └── rules.rs
└── application/
    ├── mod.rs
    ├── detector.rs
    └── rules.rs
```

Ordre de détection origine :

```text
AI coding agent
↓
IDE
↓
terminal
↓
environment
↓
unknown
```

Exemple :

```text
WindowsTerminal
└── PowerShell
    └── Codex
        └── node
```

doit donner :

```text
Origin: Codex
Via: PowerShell / Windows Terminal
```

Pas :

```text
Origin: Windows Terminal
```

Rules V0 :

```text
Codex
Claude Code
Cursor
VS Code
Windows Terminal
PowerShell
CMD
WSL
Docker
```

Puis application detection :

```text
command line contains next dev
→ Next.js

vite
→ Vite

node.exe
→ Node.js

python.exe
→ Python

cargo run / target/*.exe
→ Rust
```

Priorité :

```text
specific framework
↓
runtime
↓
raw executable
```

Confidence :

```text
Exact
High
Medium
Low
Unknown
```

Exemple :

```text
codex.exe ancestor
→ Exact/High

path happens to contain "codex"
→ Low
```

Tests obligatoires pour chaque règle.

Gate :

pas une seule règle de détection sans test.

---

# Phase 6 — Snapshot Engine

Créer le cœur opérationnel :

```rust
PortSpyEngine::snapshot()
```

Retour :

```rust
SystemSnapshot
```

Pipeline :

```text
scan listeners
        ↓
dedupe PIDs
        ↓
collect process metadata
        ↓
resolve ancestry
        ↓
detect application
        ↓
detect origin
        ↓
sort/normalize
        ↓
SystemSnapshot
```

Le TUI ne doit jamais assembler lui-même ces informations.

Il reçoit uniquement :

```rust
SystemSnapshot
```

Ajouter :

```rust
SnapshotDiff
```

pour comprendre :

```text
listener appeared
listener disappeared
process changed
```

Même si la V0 l’utilise peu, ça simplifie énormément le refresh.

Important :

un process peut disparaître entre :

```text
GetExtendedTcpTable
```

et :

```text
OpenProcess
```

C’est normal.

Le snapshot doit supporter :

```text
listener connu
process metadata partielle
```

sans échouer entièrement.

Gate :

stress-test avec créations/destructions rapides de listeners sans panic.

---

# Phase 7 — Event loop TUI

Seulement maintenant Ratatui.

Crate :

```text
portspy-tui/
```

Architecture :

```text
App
├── State
├── Event loop
├── Input mapping
├── Commands
└── Render
```

State :

```rust
AppState {
    snapshot,
    selected,
    sort,
    filter,
    mode,
    notification,
}
```

Modes :

```rust
Browse
Inspect
Search
ConfirmKill
```

Actions :

```rust
MoveUp
MoveDown
Inspect
Back
Search
Refresh
RequestKill
ConfirmKill
Cancel
Quit
```

`keymap.rs` transforme :

```text
j
↓
MoveDown
```

La UI ne connaît pas directement les touches.

Ça permettra plus tard :

```text
vim bindings
arrow keys
custom bindings
```

Refresh dans une tâche séparée.

Jamais de Win32 dans :

```rust
render()
```

Architecture :

```text
collector
   │
   │ snapshot
   ▼
channel
   │
   ▼
AppState
   │
   ▼
render
```

UI toujours responsive.

---

# Phase 8 — Design final du TUI

C’est là qu’il faut être exigeant.

Vue principale :

```text
 PORTSPY                                           8 LISTENING

 PORT     APPLICATION        ORIGIN           MEMORY       UPTIME

› :3000   Next.js            Codex             612 MB          12m
  :3001   Next.js            Claude Code       584 MB           8m
  :5173   Vite               Cursor            321 MB           1h
  :5432   PostgreSQL         Docker            148 MB           4h


 ↑↓ navigate    ↵ inspect    / search    k kill    ? help
```

Pas une boîte autour de chaque widget.

Pas 14 couleurs.

Pas de look Matrix.

Design system terminal :

```text
Primary
Secondary
Muted
Accent
Warning
Danger
Success
```

Palette construite via capacités terminal, avec fallback.

Vue Inspect en split :

```text
 PORTS                         │ :3000
                               │
 :3000 Next.js     Codex       │ Next.js
 :3001 Next.js     Claude      │ Running
 :5173 Vite        Cursor      │
                               │ PID      18432
                               │ Memory   612 MB
                               │ Uptime   12m
                               │ Scope    localhost
                               │
                               │ pnpm dev
                               │
                               │ D:\dev\bengal
                               │
                               │ Process chain
                               │ Windows Terminal
                               │ └─ PowerShell
                               │    └─ Codex
                               │       └─ Next.js
```

Responsive breakpoints.

Large :

```text
PORT APP PROJECT ORIGIN PID CPU RAM UPTIME
```

Medium :

```text
PORT APP ORIGIN RAM UPTIME
```

Small :

```text
:3000 Next.js
Codex · 612 MB · 12m
```

Tests snapshot UI si possible.

Gate manuel sur :

```text
Windows Terminal
PowerShell
cmd
terminal étroit
terminal large
resize live
```

Zéro flicker perceptible.

---

# Phase 9 — Search, sort, navigation

Search :

```text
/
```

Recherche sur :

```text
port
application
process
origin
PID
command line
```

Tri :

```text
port
memory
uptime
origin
application
```

Je ne mettrais pas 12 raccourcis.

Un menu `s` pour le tri peut être plus propre.

Sélection stable basée sur :

```text
ListenerId
```

et jamais uniquement :

```text
usize
```

Quand un port disparaît, sélectionner le voisin logique.

Pas de saut de curseur anarchique.

---

# Phase 10 — Kill process

Première action destructive.

`k` :

```text
Kill Next.js :3000 (PID 18432)?
Y confirm · Esc cancel
```

Pas de kill immédiat.

Pour V0 :

```text
kill leaf process uniquement
```

Pas encore :

```text
kill entire process tree
```

C’est plus délicat qu’il n’y paraît.

Après succès :

```text
✓ Stopped Next.js :3000
```

Puis refresh immédiat.

Cas :

```text
process already exited
```

→ succès logique / information.

Cas :

```text
permission denied
```

→ notification propre.

Cas :

```text
protected/system process
```

→ refuser par défaut.

Il faudra une policy de protection minimale.

Ne jamais permettre facilement à PortSpy de tuer :

```text
System
csrss
wininit
services
lsass
```

etc.

---

# Phase 11 — CLI propre

Même moteur, hors TUI.

```bash
portspy
```

ouvre le TUI.

Mais aussi :

```bash
portspy list
portspy inspect 3000
portspy kill 3000
```

Puis :

```bash
portspy list --json
portspy inspect 3000 --json
```

Le JSON est important dès maintenant.

Exemple :

```json
{
  "port": 3000,
  "protocol": "tcp",
  "application": "nextjs",
  "process": {
    "pid": 18432,
    "name": "node.exe"
  },
  "origin": {
    "kind": "codex",
    "confidence": "high"
  }
}
```

Le TUI et le JSON utilisent exactement les mêmes modèles core.

Aucune duplication.

---

# Phase 12 — Hardening Windows

C’est probablement la phase qui fera la différence entre prototype et vrai outil.

Tester :

```text
process disappears during scan
PID recycled
access denied
IPv6 listeners
multiple addresses same port
same PID with several ports
many listeners
localhost IPv4 + IPv6
process without command line access
admin process
Windows service
Docker Desktop
WSL
terminal closed while child survives
100+ listeners
rapid open/close
```

Ajouter des benchmarks simples :

```text
snapshot latency
process metadata collection
detection engine
TUI refresh
```

Cible raisonnable :

```text
cold snapshot < ~100-200ms typique
subsequent refresh significantly faster
TUI never blocked by scan
```

Je ne transformerais pas ces chiffres en contrat avant d’avoir mesuré plusieurs machines.

---

# Phase 13 — Observabilité interne

Ajouter `tracing`.

Exemple :

```bash
PORTSPY_LOG=debug portspy
```

Le TUI ne doit pas recevoir les logs sur stdout.

Utiliser fichier :

```text
%LOCALAPPDATA%\PortSpy\logs\
```

Plus tard.

Événements utiles :

```text
snapshot_started
snapshot_completed
listener_count
process_lookup_failed
origin_detected
kill_requested
kill_completed
```

Très pratique pour debugger les machines utilisateurs.

---

# Phase 14 — CI Windows

GitHub Actions doit obligatoirement tester Windows.

Matrix minimale :

```text
windows-latest
stable Rust
MSRV éventuelle plus tard
```

CI :

```bash
cargo fmt --check
cargo check
cargo clippy
cargo test
cargo deny check
```

Puis release build :

```bash
cargo build --release
```

Idéalement :

```text
x86_64-pc-windows-msvc
```

en premier.

ARM64 après.

---

# Phase 15 — Packaging

Première release :

```text
portspy.exe
```

Un seul binaire.

Pas d’installer MSI obligatoire au début.

Distribution :

```text
GitHub Releases
cargo install éventuellement
Scoop
Winget plus tard
```

`portspy.exe` doit fonctionner immédiatement.

Pas :

```text
install service
create database
restart PC
```

---

# Phase 16 — README release

README extrêmement simple.

Hero :

```text
PortSpy
Know what's using your localhost.
```

GIF du TUI.

Puis :

```bash
portspy
```

Exemple :

```text
:3000 Next.js  Codex
:5173 Vite     Cursor
```

Et :

```text
Press K to stop it.
```

Le README doit montrer la proposition de valeur en 10 secondes.

---

# Découpage Git que je donnerais à Codex

Je ne lui dirais surtout pas :

> “Build PortSpy.”

Je découperais le chantier en PR indépendantes :

```text
PR-001 Repository foundation
PR-002 Core domain model
PR-003 Windows TCP listener discovery
PR-004 Windows process metadata
PR-005 Process ancestry
PR-006 Origin detection engine
PR-007 Application detection engine
PR-008 System snapshot engine
PR-009 TUI event/state architecture
PR-010 Main listener table
PR-011 Inspect split view
PR-012 Search + sorting + responsive layout
PR-013 Kill process
PR-014 CLI commands + JSON
PR-015 Windows hardening
PR-016 CI + release pipeline
PR-017 v0.1 documentation
```

Chaque PR doit avoir :

```text
scope précis
tests
docs si architecture modifiée
cargo fmt
cargo clippy
cargo test
aucun unrelated refactor
```

Et surtout :

> Aucun agent ne doit profiter d’une PR pour “simplifier” arbitrairement l’architecture d’une autre couche.

---

# Definition of Done de `v0.1.0`

Je ne release pas tant que tout ça n’est pas vrai :

```text
[ ] Tous les listeners TCP Windows sont visibles
[ ] IPv4 + IPv6
[ ] PID correct
[ ] Process name correct
[ ] Process ancestry reconstruite
[ ] Codex détecté
[ ] Claude Code détecté
[ ] Cursor détecté
[ ] VS Code détecté
[ ] Windows Terminal détecté
[ ] PowerShell détecté
[ ] WSL reconnaissable
[ ] Next.js détecté
[ ] Vite détecté
[ ] Node/Python/Rust fallback
[ ] Search
[ ] Sort
[ ] Responsive layout
[ ] Auto refresh
[ ] Stable selection
[ ] Inspect view
[ ] Kill confirmation
[ ] Permission errors propres
[ ] Aucun panic connu
[ ] --json disponible
[ ] CI Windows verte
[ ] Release build reproductible
[ ] README + GIF
```

Et seulement ensuite on commence `v0.2`.

# Après v0.1

La suite logique serait :

```text
v0.2
UDP
process-tree kill
better WSL attribution
project/cwd detection
duplicate detection
orphan detection

v0.3
Docker awareness
resource history
CPU/RAM trends
service grouping

v0.4
agent CLI integration
Codex/Claude skill
MCP
ensure/reuse semantics

v1.0
cross-platform
Linux
macOS
stable JSON API
plugin/rule system
```

Le point important est que `v0.1` doit déjà être un produit que tu peux garder ouvert tous les jours. Pas une “foundation release” que personne n’utilise.

Si `portspy` te permet en deux secondes de voir `:3000 → Next.js → Codex → 612 MB` et d’appuyer sur `K`, la première version remplit déjà son contrat.
