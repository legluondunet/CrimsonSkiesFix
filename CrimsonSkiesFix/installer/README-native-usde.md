# CrimsonSkiesFix — installateur expérimental, quatre éditions

**Build expérimental Win32 x86, non destiné à une release publique.**

Ce test apporte une installation automatisée pour quatre profils de CD originaux. Le launcher vérifie l'image de CD et une copie **déjà possédée** du correctif officiel Microsoft 1.02, mais **n'exige plus d'exécutable déchiffré**.

L'installateur ne télécharge et n'exécute jamais le correctif Microsoft. Il lit le CAB intégré **en mémoire** après vérification de la taille, du SHA-256 et de sa structure ; les sources protégées 1.02 sont traitées par la bibliothèque Rust `unsafedisk` dans le même processus. Seul l'EXE 1.02 final est produit temporairement, puis activé via la transaction existante et sa sauvegarde du chargeur protégé CD. Aucun fichier propriétaire n'est embarqué.

**Important :** la voie **CD US/DE/FR_002 seul**, sans correctif, reste un chantier distinct et non résolu. Cette compilation sert d'étape fonctionnelle, et ne représente pas encore le critère final de l'issue #46.

## Éditions reconnues

| Profil | Fichiers CD | Source requise en plus du CD |
| --- | ---: | --- |
| C_SKIES_FR_001 (FR 1.02) | 112 | Aucune |
| C_SKIES_FR_002 (FR ancienne) | 107 | Correctif officiel 1.02 local |
| C_SKIES_DISC_001 (US/Europe) | 112 | Correctif officiel 1.02 local |
| C_SKIES_DE_001 (DE) | 107 | Correctif officiel 1.02 local |

Les profils sont identifiés par la totalité des chemins, tailles et SHA-256 internes. Les noms des images et leurs tailles globales ne déterminent pas l'édition. Les fichiers `.iso` peuvent contenir des secteurs classiques de 2048 octets ou des secteurs bruts MODE1/2352 et MODE2/2352 Form 1 ; leur géométrie est détectée par les descripteurs ISO9660 et les en-têtes bruts, sans conversion. Une géométrie ambiguë est refusée ; fournir alors le CUE original.

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

Le lanceur affiche le code de retour du jeu dans la console et le consigne dans un rapport non cumulatif lors d’une installation, réparation ou sortie non nulle. Les relancements ordinaires réussis préservent le dernier rapport. Un code non nul ne prouve pas à lui seul un bug. Les lancements ordinaires ne créent plus de journal de session séparé ; les anciens fichiers peuvent être supprimés manuellement. Si le lanceur est interrompu, le code de retour peut rester inconnu.

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

## Limites

- Cette compilation nécessite encore le correctif 1.02 **local** pour US/DE/FR_002. Aucun téléchargement, aucune redistribution, aucun correctif privé incorporé.
- Les essais CI vérifient la compilation Windows, le refus de CD manquant et le parcours synthétique. Les essais réels ont réussi pour FR_001 et DE dans les branches antérieures sous Wine, pour US/Europe dans la PR consolidée sous Wine, et pour FR_002 sous Windows 11 (`3b7b5597`). Les autres combinaisons d'OS/édition restent à confirmer ; ne pas déduire une validation exhaustive des essais CI.
- Les binaires non reconnus et les résultats différents du SHA-256 attendu sont rejetés.
- La PR est expérimentale. Conserver la validation FR existante et effectuer une revue technique et juridique avant toute fusion/release.
