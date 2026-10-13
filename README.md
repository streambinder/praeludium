# præludium

Diario di studio giornaliero per chi prepara concorsi d'orchestra: ogni giorno calcola cosa studiare con una formula spiegabile, traccia minuti e confidenza per pezzo, e tiene lo storico.

## Esecuzione con Docker

```sh
docker build -t praeludium .
docker run -d --name praeludium -p 127.0.0.1:8080:8080 \
  -v praeludium-data:/data \
  ghcr.io/streambinder/praeludium:latest
```

Poi apri <http://localhost:8080>. L'app non fa autenticazione: in produzione va
esposta solo dietro un reverse proxy che applica HTTP Basic Auth.

Variabili d'ambiente:

| variabile          | default  | descrizione                   |
| ------------------ | -------- | ----------------------------- |
| `PRAELUDIUM_DATA_DIR` | `/data`  | directory del database SQLite |
| `PORT`             | `8080`   | porta HTTP                    |

Sviluppo locale: `go run .` (richiede Go 1.25+).

## La formula del piano giornaliero

Per ogni pezzo, ogni giorno:

```text
punteggio = base × urgenza × bisogno × ripresa (× 1,5 se rimandato da ieri)
```

- **base** = somma dei pesi dei concorsi _futuri_ in cui compare il pezzo (i concorsi con data passata o archiviati non contano; un pezzo senza concorsi futuri esce dal piano)
- **urgenza** = 1 + k × max(0, 1 − giorni_alla_prova / orizzonte), con k=2 e orizzonte=60 giorni di default
- **bisogno** = 1 + (5 − confidenza) / 5, confidenza 1–5 dall'ultimo check-in (2,5 se mai valutata)
- **ripresa** = 1 + min(giorni_dall_ultima_seduta / 7, tetto), tetto=1 di default (spaced repetition)

I primi 8 pezzi per punteggio si dividono il budget del giorno (dalle fasce orarie libere dichiarate al mattino) in proporzione al punteggio: minimo 5 minuti, arrotondati ai 5. Ogni scheda mostra il conto completo, così è sempre chiaro _perché_ quel pezzo è lì oggi.

I coefficienti k, orizzonte e tetto si modificano da Impostazioni; i pesi dei concorsi da Concorsi.
