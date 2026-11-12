# Changelog

## 0.0.3 (in progress)

### English

- Built on fs-components v0.0.11 and fs-interfaces v0.0.19 (Blinc fs-blinc-3.1). `fs-people` stays at v0.0.12 until the People lift has a tag; until then the lock holds two fs-interfaces sources (known limit).
- The version handshake names the program (`Expected::with_program("fs-groups")`): a server of another program on the socket is not taken for this one.
- A lost connection (the server went away while the window runs) is noticed (`ApiError::Lost`); the next call connects anew, with the handshake, instead of answering "went away" until the window is reopened.
- When connecting anew fails, the first reason (for example the version that runs) is kept and shown together with the reason of the last try; it is no longer overwritten.
- For the typing test only: the optional feature `test-access` (debug builds; fs-components refuses it in a release build) lets the test read what is typed into the fields.

### Deutsch

- Gebaut auf fs-components v0.0.11 und fs-interfaces v0.0.19 (Blinc fs-blinc-3.1). `fs-people` bleibt bei v0.0.12, bis der People-Lift einen Tag hat; bis dahin stehen zwei fs-interfaces-Quellen im Lock (bekannte Grenze).
- Der Versions-Handshake nennt das Programm (`Expected::with_program("fs-groups")`): ein Server eines anderen Programms am Socket gilt nicht als dieser.
- Eine verlorene Verbindung (der Server ging weg, während das Fenster läuft) wird bemerkt (`ApiError::Lost`); der nächste Aufruf verbindet neu, mit Handshake, statt bis zum Neuöffnen des Fensters "went away" zu antworten.
- Schlägt das Neu-Verbinden fehl, bleibt der erste Grund (zum Beispiel die laufende Version) erhalten und wird mit dem Grund des letzten Versuchs gezeigt; er wird nicht mehr überschrieben.
- Nur für den Tipptest: das optionale Feature `test-access` (Debug-Builds; fs-components verweigert es in einem Release-Build) lässt den Test lesen, was in die Felder getippt wird.
