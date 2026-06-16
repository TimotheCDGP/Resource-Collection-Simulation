# Resource Collection Simulation

Simulation graphique en terminal de robots autonomes collectant des ressources sur une carte générée procéduralement. Écrit en Rust avec [Ratatui](https://ratatui.rs).

## Fonctionnalités

- Carte 80×25 générée par bruit de Perlin (obstacles)
- 5 à 10 dépôts d'énergie et 5 à 10 dépôts de cristaux (50 à 200 unités chacun)
- 3 robots éclaireurs qui explorent et signalent les ressources
- 2 robots collecteurs qui ramassent une unité à la fois et la rapportent à la base
- Architecture concurrente : un thread par robot, communication via canaux `mpsc`
- Pathfinding BFS pour contourner les obstacles
- Rendu temps réel à environ 10 images/seconde

## Légende visuelle

| Symbole | Élément | Couleur |
|---|---|---|
| ` ` | Sol libre | — |
| `O` | Obstacle | cyan clair |
| `#` | Base | vert clair |
| `E` | Dépôt d'énergie | vert |
| `C` | Dépôt de cristaux | magenta clair |
| `x` | Robot éclaireur | rouge |
| `o` | Robot collecteur | magenta |

## Compilation et exécution

### Avec Cargo

```bash
cargo run --release
```

### Avec Docker

```bash
docker build -t resource-sim .
docker run --rm -it -v "$(pwd):/workspace" resource-sim cargo run --release
```

## Contrôles

- N'importe quelle touche : quitter

## Architecture

Le code est découpé en modules pour séparer clairement les responsabilités.

```
src/
├── main.rs        — point d'entrée : setup terminal et appel à app::run
├── app.rs         — orchestration : spawn des threads, drain du canal, boucle de rendu
├── map.rs         — terrain immuable (Tile, Map, génération Perlin)
├── pathfinding.rs — helpers de navigation (marche aléatoire, BFS)
├── world.rs       — types partagés (ResourceKind, Robot, WorldState, RobotEvent)
├── robot.rs       — boucles de comportement éclaireur et collecteur
└── ui.rs          — rendu Ratatui
```

### Modèle de concurrence

- `Arc<Map>` : terrain partagé en lecture seule, immuable après génération.
- `Arc<Mutex<WorldState>>` : état dynamique (positions des robots, ressources restantes, ressources connues, totaux collectés). Les locks sont volontairement très courts.
- `mpsc::channel<RobotEvent>` : événements asynchrones des robots vers le thread principal.

Deux types d'événements transitent par le canal :

- `Discovered { pos, kind }` : un éclaireur a marché sur une ressource.
- `Collected { kind, amount }` : un collecteur a déposé à la base.

Le thread principal draine le canal de manière non bloquante (`try_recv`), met à jour `WorldState`, puis prend un snapshot pour le rendu sans conserver le lock pendant le `draw`.

### Comportements

- **Éclaireur** : marche aléatoire évitant les obstacles. Émet un événement `Discovered` à chaque ressource sur laquelle il marche.
- **Collecteur** : machine à états — cible la ressource connue la plus proche **et réellement atteignable** (filtrée par BFS), s'y rend pas à pas, ramasse une unité, retourne à la base, dépose, recommence.

## Spécification

Voir [`rust-projet.pdf`](rust-projet.pdf) pour le cahier des charges complet.
