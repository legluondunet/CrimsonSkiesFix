# Crimson Skies — vérifier la fermeture du jeu sans modifier la DLL

**Statut :** procédure préparée, **fermeture propre du vrai jeu non encore confirmée**.
Cette vérification est un critère distinct de la réussite des tests graphiques et de la validation du chargement PE32 de `ddraw.dll`.

## Ce que montrent les fichiers déjà reçus

- Le journal du test Wine/GE-Proton `CrimsonSkiesFix(20261104-151344).log`, version `0.1.90-7ee1db87`, confirme le fonctionnement du jeu, neuf générations graphiques, les retours menu et quatre allocations ROFFILE protégées, mais se termine pendant une opération graphique. Il n'y a **ni marqueur de fermeture normale ni code de sortie**. Cette fin de journal **n'implique pas** un crash.
- Un autre journal historique `crimson_skies_wine.log` contient une ancienne **exception non gérée** `c0000005` à `ROFFILE.DLL + 0x22E0` et un **code de sortie Wine/umu de 5**. Il ne concerne pas la même session que le dernier journal avec le correctif ROFFILE activé. Il ne faut pas en déduire que la version actuelle plante encore.
- `DllMain` de `ddraw.dll` est volontairement minimal et n'écrit aucun journal pendant `DLL_PROCESS_DETACH`, sous le verrou du chargeur Windows. **Ne pas ajouter une écriture, un chargement DLL, un verrou applicatif ou une attente dans DllMain** simplement pour produire un message « clean exit ». Une telle instrumentation pourrait elle-même provoquer un blocage.

## Observateur externe (Linux / Wine / Proton / Windows)

`tools/verify_game_exit.py` utilise uniquement la bibliothèque Python standard et lance le jeu en **enfant direct**, attend la fin de ce processus et enregistre :

- le code de sortie numérique et sa représentation hexadécimale ;
- la durée et le PID du processus surveillé ;
- `CrimsonSkiesFix-exitcheck.stdout.log` et `CrimsonSkiesFix-exitcheck.stderr.log` ;
- un rapport `CrimsonSkiesFix-exitcheck.report.txt` contenant une classification factuelle ;
- les messages qui signalent **explicitement une exception non gérée** dans Wine/Proton, ainsi que le code de sortie du jeu éventuellement signalé par `umu`.

Il **n'injecte rien** dans le jeu, ne remplace pas le proxy, ne patche aucune fonction et n'altère aucun fichier original. Les trois rapports sont écrits dans le dossier fourni via `--report-dir` (par défaut dans le répertoire du jeu), en écrasant les précédents rapports de même nom.

### Exécution sous Manjaro / Wine / GE-Proton

Utiliser **le même runner, le même préfixe Wine et le même environnement** que lors des précédents tests validés. Ne pas substituer arbitrairement le Wine du système à GE-Proton.

```sh
python3 /chemin/vers/CrimsonSkiesFix/tools/verify_game_exit.py \
  --cwd "/chemin/vers/Crimson Skies" \
  --report-dir "/chemin/vers/rapports" \
  -- /chemin/vers/le/runner/wine CRIMSON.EXE
```

Le `--` sépare les options de l'observateur de la commande réelle. Le lancement doit attendre le **processus du jeu**. Un bouton de démarrage Lutris, une application Steam ou un shell qui **détache immédiatement** le processus ne permet pas de conclure sur sa fermeture, même avec un code `0`.

Faire idéalement deux essais : fermer depuis le menu principal avec **Quit**, puis lancer une mission, revenir au menu et quitter. Ne pas fermer de force l'application ni arrêter `wineserver` pour ces tests.

### Exécution sous Windows 11

Depuis un terminal Python, lancer directement `CRIMSON.EXE` avec un chemin absolu :

```powershell
python .\CrimsonSkiesFix\tools\verify_game_exit.py --cwd "C:\Games\Crimson Skies" -- "C:\Games\Crimson Skies\CRIMSON.EXE"
```

Ce chemin est un **exemple**. Le script doit être accessible à son emplacement réel sur l'ordinateur. Ce contrôle Windows reste en attente d'une machine Windows et ne doit jamais être décrit comme déjà validé.

## Interprétation

| `Assessment` | Interprétation |
| --- | --- |
| `ZERO_PROCESS_EXIT_NO_UNHANDLED_EXCEPTION_FOUND` | Processus surveillé terminé avec `0`, sans marqueur explicite d'exception non gérée dans ses sorties : **indice fort de fermeture correcte**, pas preuve absolue |
| `CRASH_EVIDENCE_IN_PROCESS_OUTPUT` | Les sorties surveillées contiennent un marqueur explicite de type `Unhandled exception` ou `segmentation fault` |
| `CRASH_LIKELY_WINDOWS_EXCEPTION` | Code de sortie Windows NTSTATUS caractéristique d'un crash, par exemple `0xC0000005` |
| `PROCESS_ENDED_BY_POSIX_SIGNAL` | Signal POSIX reçu, potentiellement dû à un crash ou à une interruption externe |
| `NONZERO_PROCESS_EXIT_NEEDS_REVIEW` | Code de sortie non nul, qui **ne prouve pas seul** un crash |
| `NONZERO_GAME_EXIT_REPORTED_BY_LAUNCHER` | Lanceur `umu` rapporte un code non nul du jeu, même si le processus directement surveillé a renvoyé zéro |

Le programme peut se terminer avec `0` malgré une erreur interne gérée; inversement, un jeu ancien peut renvoyer un code non nul après une sortie volontaire. En cas de doute, examiner les trois rapports et les événements de fermeture visibles. **Aucun test unitaire simulant la fin d'un faux processus ne prouve que Crimson Skies se ferme correctement.**

## Intégration CI et protocole de décision

Le workflow GitHub Actions exécute le `--self-test` de l'observateur sur Windows et Linux. Il teste la distinction entre sortie zéro, sortie non nulle, message d'exception non gérée, code de sortie du lanceur, exception Windows et signal POSIX **sans jeu ni interface graphique**.

**Critère utilisateur restant :** deux fermetures volontaires du jeu sous Wine/GE-Proton avec le binaire exact, sans écran de plantage, puis fournir uniquement le `CrimsonSkiesFix-exitcheck.report.txt` et, si un résultat est douteux, les sorties stdout/stderr correspondantes. Les trois fichiers peuvent contenir des chemins ou des informations système à examiner avant partage.

Si les deux tests passent, enregistrer les SHA du proxy et du jeu, et marquer la fermeture Wine comme validée. La fermeture sous Windows 11 reste un critère **séparé**, à exécuter lorsque l'ordinateur Windows sera disponible.

legluondunet assisted by ChatGPT (OpenAI)
