import requests


class LLMProcessor:
    def __init__(self, model: str = "llama3.2:3b", base_url: str = "http://localhost:11434"):
        self.model = model
        self.base_url = base_url

    def process(self, text: str, prompt_template: str) -> str:
        if not text:
            return text

        full_prompt = f"{prompt_template}\n\n{text}"
        try:
            response = requests.post(
                f"{self.base_url}/api/generate",
                json={"model": self.model, "prompt": full_prompt, "stream": False},
                timeout=(2, 30),
            )
            response.raise_for_status()
            result = response.json().get("response", "").strip()
            return result if result else text
        except requests.exceptions.ConnectionError:
            print("[LLM] Ollama non disponible — texte retourné sans correction.")
            return text
        except Exception as e:
            print(f"[LLM] Erreur : {e}")
            return text

