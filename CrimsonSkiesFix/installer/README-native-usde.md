# CrimsonSkiesFix — installateur expérimental, cinq éditions

**Build expérimental Win32 x86, non destiné à une release publique.**

Branche 1.2 dev.3 : pilotage SDL3, mappage d'actions fixe et premier essai de navigation. Gauche roulis/tangage, droite lacet/gaz (gaz modifiés uniquement dans le traitement réel du pilote ; initialisation à 100 % lors d'un nouvel avion joueur observé). RT canons, LT roquettes, RB ennemi suivant, LB précédent, A changer de canon, B nitro, X vue, Y cible sous le viseur, L3 changer de roquette, Start carte/pause. Menu 800x600 : stick gauche/croix déplacent le curseur natif, A clic, B Échap dans les scripts qui lisent cette touche. Le mappage ne dépend pas des affectations joystick sauvegardées. Activer [Debug] Log=1 pour tester. Pas de gyroscope, pavé tactile, LED ou vibration active.

Ce test apporte une installation automatisée pour cinq profils de CD originaux. Le launcher vérifie l'image de CD et une copie **déjà possédée** du correctif officiel Microsoft 1.02, mais **n'exige plus d'exécutable déchiffré**.

L'installateur ne télécharge et n'exécute jamais le correctif Microsoft. Il lit le CAB intégré **en mémoire** après vérification de la taille, du SHA-256 et de sa structure ; les sources protégées 1.02 sont traitées par la bibliothèque Rust `unsafedisk` dans le même processus. Seul l'EXE 1.02 final est produit temporairement, puis activé via la transaction existante et sa sauvegarde du chargeur protégé CD. Aucun fichier propriétaire n'est embarqué.

**Décision de périmètre :** les CD US/DE/FR_002 nécessitent un correctif 1.02 externe. La reconstruction 1.00 vers 1.02 est suspendue ; l'utilisateur télécharge le correctif une fois si nécessaire. L'installation elle-même reste hors ligne.

## Éditions reconnues

| Profil | Fichiers CD | Source requise en plus du CD |
| --- | ---: | --- |
| C_SKIES_FR_001 (FR 1.02) | 112 | Aucune |
| C_SKIES_FR_002 (FR ancienne) | 107 | Correctif officiel 1.02 local |
| C_SKIES_DISC_001 (US/Europe) | 112 | Correctif officiel 1.02 local |
| C_SKIES_DE_001 (DE) | 107 | Correctif officiel 1.02 local |
| C_SKIES_DISC_002 (Europe anglais 1.02) | 118 | Aucune — validé sous Wine 11.19 |

Les profils sont identifiés par la totalité des chemins, tailles et SHA-256 internes. Les éditions non officielles, repacks et traductions non authentifiées ne sont pas pris en charge. Les noms des images et leurs tailles globales ne déterminent pas l'édition. L'extraction vérifie de nouveau le profil complet et refuse tout changement d'édition depuis la sélection du parcours d'installation. Pour les deux CD déjà en 1.02, un correctif externe éventuellement présent n'est pas nécessaire.

## Si le correctif est demandé

Le terminal identifie le CD reconnu et explique pourquoi la version 1.02 est indispensable. Les fenêtres sont concises, sans identifiant de profil ni chemin absolu. Pour un correctif manquant ou invalide, une boîte de dialogue Win32 propose d'ouvrir la section de téléchargement du README dans le navigateur par défaut (Oui), ou de fermer le message (Non). Si plusieurs copies valides sont présentes, conserver un seul exemplaire. Les autres erreurs bloquantes de l'installateur affichent leur diagnostic. Aucun dialogue en cas de succès. Les erreurs bloquantes sont affichées à la fois dans le terminal et dans une fenêtre, avec ou sans arguments, sans détection du terminal. Seuls la CI et le mode non interactif désactivent la fenêtre. Pour tout script ou lancement non surveillé, définir `CRIMSONSKIESFIX_NONINTERACTIVE=1` pour garantir l'absence de dialogue bloquant. Télécharger `crmsk102.exe` depuis une seule de ces sources : [Releases du projet](https://github.com/legluondunet/CrimsonSkiesFix/releases) s'il y est proposé, [MyLittleLutrisScripts (archive 7z)](https://github.com/legluondunet/MyLittleLutrisScripts/raw/refs/heads/master/Crimson%20Skies/crmsk102.7z), [PCGamingWiki](https://community.pcgamingwiki.com/files/file/62-crimson-skies-patch/), [ModDB](https://www.moddb.com/games/crimson-skies/downloads/crimson-skies-update-v102), ou [My Abandonware](https://www.myabandonware.com/game/crimson-skies-be7) (Extras, International patch v1.02 uniquement). Si le téléchargement est une archive 7z ou ZIP, extraire d'abord l'EXE. Placer un seul exemplaire authentique à côté du lanceur (pas dans game/), ne pas l'exécuter, puis relancer CrimsonSkiesFix.exe. Le fichier est vérifié et appliqué automatiquement. Aucun EXE déchiffré séparé n'est demandé.

## Test d'installation neuve

1. Créer un répertoire de test **distinct** de l'installation française validée ou de toute sauvegarde existante.
2. Y placer `CrimsonSkiesFix.exe`, une image du **CD original reconnu** (`.iso` ou `.cue` avec `.bin`/`.img`), ainsi que votre **copie authentique** du correctif Microsoft 1.02 si le profil l'exige. Le nom du correctif est libre : seul son contenu SHA-256 est utilisé.
3. **Ne pas ajouter d'EXE déchiffré**. La reconstruction est réalisée par l'installateur.
4. Exécuter `CrimsonSkiesFix.exe --version` et vérifier `NATIVE US/DE LOCAL UPDATE 1.02 EXPERIMENTAL`.
5. Exécuter `CrimsonSkiesFix.exe --install-only`, puis `CrimsonSkiesFix.exe --verify` ; ensuite lancer `CrimsonSkiesFix.exe` pour jouer.
6. En cas de problème, conserver `CrimsonSkiesFix_installer.log`,
   et, si `[Debug] Log=1`,
   `CrimsonSkiesFix.log` au même niveau que le lanceur.

## Diagnostic des sorties anormales

Le lanceur affiche le code de retour du jeu dans la console et le consigne dans un rapport non cumulatif lors d’une installation, réparation ou sortie non nulle. Le rapport identifie aussi la compilation, le profil vérifié (`Edition`) et la source du correctif consignée dans le reçu (`RecordedUpdateSource`). Une source non reconnue ou absente est signalée `UNKNOWN` ; cette information n'est pas un nouveau contrôle d'intégrité. Les relancements ordinaires réussis préservent le dernier rapport. Un code non nul ne prouve pas à lui seul un bug. Les lancements ordinaires ne créent plus de journal de session séparé ; les anciens fichiers peuvent être supprimés manuellement. Si le lanceur est interrompu, le code de retour peut rester inconnu.

Pour obtenir le journal détaillé du runtime sans changer les réglages
graphiques, définir temporairement `[Debug] Log=1` dans le fichier
`CrimsonSkiesFix.ini` existant (ne pas supprimer les autres paramètres).
Le journal de jeu `CrimsonSkiesFix.log` est recréé à chaque session.
`Log=0` est la valeur par défaut volontaire. Un INI créé par l'installateur active le journal pour la première session ; après une sortie normale avec un journal non vide, il passe à `Log=0` si l'utilisateur n'a pas modifié ses réglages. Le marqueur interne d'éclairage High est conservé.

Les relevés d'appels de rendu `RENDER-SAMPLE` et `RENDER-WINDOW`
sont réservés au diagnostic approfondi : ajouter temporairement
`FrameTimeTrace=1` dans `[Debug]`, avec `Log=1`. Ils ne constituent
**pas** une mesure des FPS réellement affichés. Sans ce drapeau,
`Log=1` conserve les diagnostics de rendu, de menu et de stabilité
sans ces échantillons répétitifs.

Sous Wine, pour diagnostiquer un plantage qui ferme aussi le lanceur,
exécuter depuis un terminal dans le dossier du lanceur :

```sh
WINEDEBUG=-all,+seh wine CrimsonSkiesFix.exe > CrimsonSkiesFix_wine-seh.log 2>&1
```

Ce fichier Wine n'est pas produit par le jeu et peut être volumineux.
Il complète le journal du runtime, sans prétendre être un minidump. Conserver
également l'heure approximative et l'action (menu, mission, Alt-Tab, etc.)
qui a précédé le problème.

Le correctif reconnu a une taille de 2 315 728 octets et une empreinte
SHA-256 `8e1c04f3e32c318a58ef3e2bbd8f13b9894ff055d4492f80ee14ce53851616af`.
Les données exécutables finales sont validées sur le SHA-256 officiel de notre référence 1.02 `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75`.

La source CD protégée est conservée en `game/CRIMSON.EXE.protected.bak`. Les données sauvegardées `SavedGames/` et `Planes/` et le mécanisme de rollback restent pris en charge. Le jeu et `fix.dll` résident dans `game/`, avec `state/`, `docs/` et les logs à côté du lanceur.

Le **CD français révision 1.02** (`C_SKIES_FR_001`, 112 fichiers) reste utilisable sans correctif externe. Le nouveau profil expérimental `C_SKIES_FR_002` (107 fichiers, ancien ICD, CRIMPTCH.ROF absent) nécessite le correctif local comme US/DE. Une première installation réelle suivie d’un lancement sous Windows 11 a réussi avec `3b7b5597` (code de sortie 0). Un CD non reconnu est refusé avec une erreur d'identification explicite, sans demander inutilement un EXE.

Le candidat `C_SKIES_DISC_002` a été inventorié intégralement. Son `SETUP.EXE` confirme les 40 noms longs de FR_001, son ICD correspond à la référence anglaise 1.02 et son chargeur exact est reconnu par la bibliothèque épinglée. Aucun fichier propriétaire n'est ajouté au projet. Le test réel `771b566e` sous Wine 11.19 a confirmé une installation neuve depuis le CD seul, 178 ZBD extraits, aucun alias restant, exécutable vérifié et sortie 0. Le reçu confirme `C_SKIES_DISC_002` et `NOT_REQUIRED_ORIGINAL_EU_102_CD`. La confirmation sous Windows natif reste à réaliser.

## Limites

- Cette compilation nécessite encore le correctif 1.02 **local** pour US/DE/FR_002. Aucun téléchargement, aucune redistribution, aucun correctif privé incorporé.
- Les essais CI vérifient la compilation Windows, le refus de CD manquant et le parcours synthétique. Les essais réels ont réussi pour FR_001 et DE dans les branches antérieures sous Wine, pour US/Europe dans la PR consolidée sous Wine, et pour FR_002 sous Windows 11 (`3b7b5597`). Les autres combinaisons d'OS/édition restent à confirmer ; ne pas déduire une validation exhaustive des essais CI.
- Les binaires non reconnus et les résultats différents du SHA-256 attendu sont rejetés.
- La PR est expérimentale. Conserver la validation FR existante et effectuer une revue technique et juridique avant toute fusion/release.

Dev.4: automatic active gamepad selection. All mapped controllers monitored (max 32); an intentional stick/trigger/button gesture after neutral selects the first controller. Another controller takes over only after the current one is neutral for 350 ms and a new intentional gesture occurs. Disconnect releases the selected device and waits for activity on another. No Start activation. Log=1 also records bounded GOS page resource tokens to identify the original Joystick settings screen; no visual replacement yet. Enter Joystick preferences and return in the test.

Dev.5: A, B or Start can request skip in the original FMV reader, including before MainMenu. One press per clip request; release before another press. Native keyboard unchanged. In-engine scripted cinematics are not yet separately validated.

Dev.6: extend A/B/Start skip to the original in-flight 3D animation termination query. The original FMV hook remains, with one shared consumed edge per poll. Test the zeppelin sequence; native animation cleanup is unchanged. No menu replacement yet.

Dev.7 controls-panel preview: enter Preferences > Controls, hold Y/Triangle (without other controls) or F1 to see the static 4:3 flight-controls diagram, release to return to the original controls preferences. A small hint is composited on this page. No settings disabled or changed; English preview labels, original keyboard and mouse settings remain available. Check screenshots and verify the diagram never appears in other menus/flight. Log=1 emits unique Preferences page members and a presentation marker.

Dev.8: press Triangle on PlayStation or Y on Xbox once in Preferences > Controls; release does not close. The next distinct gamepad button/axis gesture, keyboard press, or mouse movement/click closes the diagram. Dismissal is consumed in the scoped GOS input paths. No F1 shortcut. Symmetric PlayStation and offset Xbox vector layouts; unknown devices use positional labels. English/French/German based on original Controls title or installer edition receipt, English fallback. The invite is below the native Apply/Reset bar. Existing keyboard/mouse preferences remain available.
