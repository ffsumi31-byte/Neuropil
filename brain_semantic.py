import numpy as np
import requests
import wikipedia
from sentence_transformers import SentenceTransformer

RUST_API = "http://127.0.0.1:8080/api/learn"
model = SentenceTransformer("all-MiniLM-L6-v2")


def project_to_3d(embeddings: np.ndarray, scale: float = 250.0) -> np.ndarray:
    """Projects high-dimensional embeddings into 3D coordinates via SVD."""
    centered = embeddings - np.mean(embeddings, axis=0)
    _, _, vt = np.linalg.svd(centered, full_matrices=False)
    coords_3d = centered @ vt[:3].T
    return coords_3d * scale


def ingest_semantic_cluster(seed_topics: list[str]) -> None:
    pages = []
    for topic in seed_topics:
        try:
            page = wikipedia.page(topic, auto_suggest=False)
            pages.append((page.title, page.summary[:350]))
        except Exception:
            continue

    titles = [p[0] for p in pages]
    summaries = [p[1] for p in pages]

    # Generate neural embeddings and 3D spatial coordinates
    embeddings = model.encode(summaries, normalize_embeddings=True)
    coords = project_to_3d(embeddings)

    nodes = []
    for idx, (title, summary) in enumerate(pages):
        nodes.append({
            "id": title,
            "group": (idx % 4) + 1,
            "summary": summary,
            "fx": float(coords[idx][0]),
            "fy": float(coords[idx][1]),
            "fz": float(coords[idx][2]),
        })

    # Connect concepts whose cosine similarity exceeds the synaptic threshold
    links = []
    similarity_matrix = embeddings @ embeddings.T
    for i in range(len(titles)):
        for j in range(i + 1, len(titles)):
            sim = float(similarity_matrix[i][j])
            if sim >= 0.38:
                links.append({
                    "source": titles[i],
                    "target": titles[j],
                    "weight": round(sim, 3),
                })

    response = requests.post(RUST_API, json={"nodes": nodes, "links": links}, timeout=10)
    print(f"Mapped {len(nodes)} cortical nodes and {len(links)} synapses: {response.text}")


if __name__ == "__main__":
    corpus = [
        "Artificial neural network",
        "Synaptic plasticity",
        "Connectome",
        "Transformer (deep learning architecture)",
        "Hippocampus",
        "Graph theory",
        "Vector database",
        "Rust (programming language)",
    ]
    ingest_semantic_cluster(corpus)
