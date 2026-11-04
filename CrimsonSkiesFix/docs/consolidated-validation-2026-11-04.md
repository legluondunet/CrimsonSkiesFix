# CrimsonSkiesFix — validation consolidée de stabilité et de synchronisation

**Base :** `main` après PR #17, commit `fd8211ef9c766e04af8be6332a3dcb5ea9772210`.
**But :** maximiser la validation automatique sans réclamer un test de jeu à chaque modification. Une compilation native Windows ne remplace jamais un essai utilisateur de Crimson Skies.

## Ce qui est maintenant autonome dans la CI

1. **Rust x86** : tous les tests de `ddraw.dll` et les tests de l'utilitaire de diagnostic, y compris l'ABI des hooks DirectDraw/Direct3D3, les références COM, le graphe Primary→Backbuffer→Z32 et les transactions de rendu.
2. **Concurrence ciblée :** le test synthétique `retirement_waits_for_inflight_flip_and_disarms_old_primary` lance un Flip sur un thread avec un Blt COM simulé temporairement lent. Pendant l'appel natif, le second thread retire la génération. Le retrait doit attendre le Flip actif; après retraite, aucune nouvelle présentation ne doit atteindre l'ancienne surface. Toutes les références natives reviennent à leur compte initial.
3. **Chargement natif 32 bits :** compilation d'un petit exécutable `ddraw_loader_smoke.exe`, exécuté sur le runner Windows. Il charge le `ddraw.dll` issu de la même compilation via `LoadLibraryW`, vérifie `DirectDrawCreate`, `DirectDrawCreateEx` et `DirectDrawEnumerateA`, puis le décharge avec `FreeLibrary`. Il ne charge **pas** Crimson Skies et ne lance **pas** Direct3D/HAL.
4. **Distribution :** PE32 `IMAGE_FILE_MACHINE_I386`, archive contenant uniquement la DLL, un modèle d'INI et le README. Les préférences validées par défaut (FOV, verrouillage vidéo, correctif écran blanc, ROFFILE sous Wine) restent imposées par la CI.

## Audit de réentrance — portée du changement

L'adaptateur conserve volontairement le verrou global `RUNTIME` pendant la transaction délicate `SetDisplayMode` / création des surfaces / retrait de l'ancien graphe : libérer ce verrou sans épingler **toutes** les références natives et virtuelles risquerait de créer un use-after-free.

Une amélioration conservatrice retire **seulement l'écriture du diagnostic `GetDisplayMode`** de la région critique, après avoir réécrit le descripteur sous le même verrou et copié son `Mode`. Les résultats COM, les valeurs du descripteur et le journal restent identiques.

**Non démontré :** l'absence générale d'interblocage en cas d'appel COM/Win32 réentrant pendant `prepare_epoch`. Les tests synthétiques n'utilisent pas les vraies fenêtres Wine, n'émulent pas toutes les règles de thread/apartment COM et ne vérifient pas les pertes matérielles de périphérique.

## Campagnes regroupées

| Campagne | Exécution sans intervention | Vérification demandée au propriétaire |
|---|---|---|
| Synchronisation COM / retrait des générations | Tests Rust x86 + test de concurrence avec Blt natif simulé + audit de code des verrous | Un unique essai sous Wine si la PR modifie l'exécution |
| Régression Wine | Tests structurels, paramètres INI, compilation et contrôle du chargement de la DLL | Un seul essai regroupant écran initial, vidéo, menu, préférences, jeu, plusieurs 1920×1080↔800×600, Alt+Tab et fermeture |
| Compatibilité Windows | Compilation et chargement de DLL x86 sur runner Windows, contrôle PE et exports | Plus tard, un essai réel du jeu sous Windows 11 sur un ordinateur adapté; le runner GitHub Actions n'est pas une validation de gameplay Windows 11 |

### Scénario Wine final (une session, pas un test par PR)

- Confirmer que la première ligne du journal porte **exactement le SHA de la PR courante**.
- Vérifier le lancement sans choix de pilote/résolution, et la bonne image initiale.
- Vérifier les cinématiques en 4:3, le menu principal, la signature qui disparaît dans les préférences et revient au menu.
- Vérifier les paramètres Graphique/Résolution verrouillés et le texte de résolution réelle, sans changer les réglages du jeu.
- Jouer en 3D plein écran sans déformation du HUD ni mauvais FOV; passer aux menus puis revenir en jeu plusieurs fois.
- Essayer au moins deux Alt+Tab, dont un pendant le jeu; vérifier absence d'écran blanc et d'instabilité.
- Vérifier fin de session normale. Si une anomalie se produit, conserver le journal et indiquer seulement l'action exacte qui l'a déclenchée.

### Scénario Windows 11 ultérieur

Avec la **même version compilée** et un exécutable authentifié : répéter le scénario ci-dessus, comparer l'affichage DirectDraw HAL, la vitesse de jeu et les cinématiques, puis consigner un journal de fermeture. La CI Windows x86 valide déjà le chargement de DLL, mais aucune session du jeu sous Windows 11 n'est disponible.

**Politique de livraison :** PRs isolées, deux workflows Windows x86 verts, revue, jamais de fusion automatique dans `main`, et pas de nouvelle release déclarée stable sans test en jeu réel.

legluondunet assisted by ChatGPT (OpenAI)
