import os
import traceback
import requests
import wikipedia

BASE_URL = os.getenv("NEUROPIL_URL", "http://127.0.0.1:8080")


def feed_wikipedia_to_workspace(topic: str) -> None:
    try:
        try:
            page = wikipedia.page(topic, auto_suggest=False)
        except wikipedia.exceptions.DisambiguationError as e:
            page = wikipedia.page(e.options[0], auto_suggest=False)

        summary = page.summary[:180].replace("\n", " ")
        payload = {
            "prompt": f"Build sharp structure for {page.title}: {summary}",
            "workspace": [
                {
                    "name": "Origin_Cube",
                    "role": f"Wikipedia Seed: {page.title}",
                    "pos": [0.0, 0.5, 0.0],
                    "size": [1.0, 1.0, 1.0],
                    "rot": [0.0, 0.0, 0.0],
                    "color": "#5EB58B",
                    "mat": "matte",
                }
            ],
        }

        res = requests.post(f"{BASE_URL}/api/prompt", json=payload, timeout=10)
        res.raise_for_status()
        print(f"[OK] Connected '{page.title}' -> AI Plan:", res.json())

    except Exception as exc:
        print("=" * 60)
        print(f"[NEUROPIL PYTHON CONNECTOR ERROR] Topic: {topic}")
        print(f"Reason: {exc}")
        print(traceback.format_exc())
        print("=" * 60)


if __name__ == "__main__":
    feed_wikipedia_to_workspace(" Brutalist architecture")
    
