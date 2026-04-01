#!/usr/bin/env python3
"""Buddy Collection — lightweight API server backed by buddies.db."""

import json
import sqlite3
import os
from http.server import HTTPServer, SimpleHTTPRequestHandler
from urllib.parse import urlparse, parse_qs

DB_PATH = os.path.join(os.path.dirname(__file__), "..", "buddies.db")
WEB_DIR = os.path.dirname(__file__)


def get_db():
    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row
    return conn


class BuddyHandler(SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=WEB_DIR, **kwargs)

    def do_GET(self):
        parsed = urlparse(self.path)
        if parsed.path == "/api/buddies":
            self._handle_buddies(parse_qs(parsed.query))
        elif parsed.path == "/api/stats":
            self._handle_stats()
        elif parsed.path == "/api/filters":
            self._handle_filters()
        else:
            super().do_GET()

    def _json_response(self, data, status=200):
        body = json.dumps(data, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(body)

    def _handle_filters(self):
        conn = get_db()
        species = [r[0] for r in conn.execute("SELECT DISTINCT species FROM buddies ORDER BY species")]
        rarities = [r[0] for r in conn.execute(
            "SELECT DISTINCT rarity FROM buddies ORDER BY CASE rarity "
            "WHEN 'legendary' THEN 0 WHEN 'epic' THEN 1 WHEN 'rare' THEN 2 "
            "WHEN 'uncommon' THEN 3 ELSE 4 END"
        )]
        hats = [r[0] for r in conn.execute("SELECT DISTINCT hat FROM buddies ORDER BY hat")]
        eyes = [r[0] for r in conn.execute("SELECT DISTINCT eye FROM buddies ORDER BY eye")]
        conn.close()
        self._json_response({"species": species, "rarities": rarities, "hats": hats, "eyes": eyes})

    def _handle_buddies(self, params):
        conn = get_db()
        conditions = []
        values = []

        for key in ("species", "rarity", "hat", "eye"):
            if key in params and params[key][0]:
                conditions.append(f"{key} = ?")
                values.append(params[key][0])

        if "shiny" in params and params["shiny"][0] == "1":
            conditions.append("shiny = 1")

        where = (" WHERE " + " AND ".join(conditions)) if conditions else ""
        order = (
            " ORDER BY CASE rarity WHEN 'legendary' THEN 0 WHEN 'epic' THEN 1 "
            "WHEN 'rare' THEN 2 WHEN 'uncommon' THEN 3 ELSE 4 END, species"
        )

        limit = min(int(params.get("limit", [60])[0]), 200)
        offset = int(params.get("offset", [0])[0])

        count_row = conn.execute(f"SELECT COUNT(*) FROM buddies{where}", values).fetchone()
        total = count_row[0]

        rows = conn.execute(
            f"SELECT user_id, species, rarity, eye, hat, shiny, "
            f"debugging, patience, chaos, wisdom, snark, sprite "
            f"FROM buddies{where}{order} LIMIT ? OFFSET ?",
            values + [limit, offset],
        ).fetchall()

        buddies = [
            {
                "user_id": r["user_id"],
                "species": r["species"],
                "rarity": r["rarity"],
                "eye": r["eye"],
                "hat": r["hat"],
                "shiny": bool(r["shiny"]),
                "stats": {
                    "debugging": r["debugging"],
                    "patience": r["patience"],
                    "chaos": r["chaos"],
                    "wisdom": r["wisdom"],
                    "snark": r["snark"],
                },
                "sprite": r["sprite"],
            }
            for r in rows
        ]
        conn.close()
        self._json_response({"total": total, "buddies": buddies, "limit": limit, "offset": offset})

    def _handle_stats(self):
        conn = get_db()
        total = conn.execute("SELECT COUNT(*) FROM buddies").fetchone()[0]
        shiny = conn.execute("SELECT COUNT(*) FROM buddies WHERE shiny=1").fetchone()[0]

        by_rarity = {
            r[0]: r[1]
            for r in conn.execute("SELECT rarity, COUNT(*) FROM buddies GROUP BY rarity")
        }
        by_species = {
            r[0]: r[1]
            for r in conn.execute("SELECT species, COUNT(*) FROM buddies GROUP BY species ORDER BY COUNT(*) DESC")
        }
        conn.close()
        self._json_response({
            "total": total,
            "shiny": shiny,
            "by_rarity": by_rarity,
            "by_species": by_species,
        })

    def log_message(self, format, *args):
        if "/api/" in str(args[0]):
            super().log_message(format, *args)


if __name__ == "__main__":
    port = int(os.environ.get("PORT", 3456))
    server = HTTPServer(("0.0.0.0", port), BuddyHandler)
    print(f"\n  🐾 Buddy Collection running at http://localhost:{port}\n")
    server.serve_forever()
