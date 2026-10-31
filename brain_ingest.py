import requests
import wikipedia

RUST_API = "http://127.0.0.1:8080/api/learn"

def learn_topic(topic: str, max_connections: int = 8) -> None:
    try:
        page = wikipedia.page(topic, auto_suggest=False)
    except wikipedia.exceptions.DisambiguationError as e:
        page = wikipedia.page(e.options[0], auto_suggest=False)

    nodes = [{
        "id": page.title,
        "group": 1,
        "summary": page.summary[:200]
    }]
    links = []

    for linked_title in page.links[:max_connections]:
        nodes.append({
            "id": linked_title,
            "group": 2,
            "summary": f"Connected concept to {page.title}"
        })
        links.append({
            "source": page.title,
            "target": linked_title,
            "weight": 1.0
        })

    payload = {"nodes": nodes, "links": links}
    response = requests.post(RUST_API, json=payload, timeout=5)
    print(f"Learned '{page.title}': {response.text}")

if __name__ == "__main__":
    seed_topics = ["Artificial intelligence", "Neuroscience", "Knowledge graph"]
    for seed in seed_topics:
        learn_topic(seed)
