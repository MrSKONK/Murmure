"""Sonde de développement : python tools/grammalecte_probe.py <dossier extrait de Grammalecte>
Affiche l'API disponible et les erreurs relevées sur des phrases dictées typiques (calibrage de src/grammar.py)."""
import json
import sys
import time

sys.path.insert(0, sys.argv[1])
sys.path.insert(0, ".")
t = time.time()
import grammalecte  # noqa: E402

checker = grammalecte.GrammarChecker("fr")
gce = checker.getGCEngine()
print("init_s", round(time.time() - t, 2))
print("version", getattr(gce, "version", "?"), getattr(grammalecte, "__version__", ""))
print("checker_api", [n for n in dir(checker) if not n.startswith("_")])
print("gce_api", [n for n in dir(gce) if not n.startswith("_")])
try:
    print("options", json.dumps(gce.getOptions(), ensure_ascii=False))
except Exception as e:
    print("options_err", e)
try:
    print("option_labels", json.dumps(gce.getOptionsLabels("fr"), ensure_ascii=False)[:4000])
except Exception as e:
    print("labels_err", e)

SAMPLES = [
    "Les enfant sont partis ce matin.",
    "Il a manger une pomme.",
    "Je suis aller au marché hier.",
    "Ils mange des pommes.",
    "Il va a Paris demain.",
    "Ou est tu passé ?",
    "Les voitures rouge sont garé devant chez moi.",
    "Elle se sont trompés de route.",
    "Je pense qu'il faut qu'on se voit demain.",
    "Ces un bon moyen de le faire.",
    "On a reçu les documents que tu nous a envoyé.",
    "Merci pour ton retour, je regarde ça et je te redis.",
    "Je voulais te dire que la réunion est décalée à 14 h.",
    "Kubernetes tourne sur le serveur de Nathan.",
    "Peux-tu me rappeler demain matin pour faire le point sur le projet Murmure ?",
    "Nous nous sommes vus à Lyon avec Marie-Claire.",
    "Écris à nathan@gmail.com et va sur www.exemple.fr.",
    "C'est quoi le problème avec le build ?",
]
for s in SAMPLES:
    t = time.time()
    gram, spell = checker.getParagraphErrors(s, bSpellSugg=False)
    ms = round((time.time() - t) * 1000)
    out = [{"t": e.get("sType"), "rule": e.get("sRuleId"), "span": s[e["nStart"]:e["nEnd"]], "sugg": e.get("aSuggestions"),
            "msg": e.get("sMessage", "")[:90]} for e in gram]
    sp = [s[e["nStart"]:e["nEnd"]] for e in spell]
    print("CASE", json.dumps({"text": s, "ms": ms, "gram": out, "spell": sp}, ensure_ascii=False))

phonet = gce.phonet
print("phonet_api", [n for n in dir(phonet) if not n.startswith("_")])
for a, b in [("mange", "mangent"), ("sont", "est"), ("Elle", "Elles"), ("a", "à"), ("aller", "allé"), ("Ou", "Où"),
             ("Ces", "C'est"), ("rouge", "rouges"), ("Ils", "Il"), ("envoyé", "envoyés"), ("a", "as"), ("vert", "verre")]:
    try:
        print("SIMIL", a, b, phonet.isSimilAs(a, b), phonet.isSimilAs(b, a))
    except Exception as e:
        print("SIMIL_ERR", a, b, e)

# Chaîne complète de src/grammar.py sur les mêmes phrases
from src import grammar  # noqa: E402
checker.getGCEngine().setOptions({k: False for k in grammar.OFF_OPTIONS if k in gce.getOptions()})
same = grammar._homophone_test(gce)
print("homophone_test", same is not None)
for s in SAMPLES + ["Elles se sont trompé de route.", "Je les est vu hier.", "Tu peux venir a 15 h ?",
                    "Il faut que tu me dise si sa marche.", "On c'est vu la semaine dernière."]:
    gram, _ = checker.getParagraphErrors(s, bSpellSugg=False)
    print("FIX", json.dumps({"in": s, "out": grammar.apply_corrections(s, gram, same)}, ensure_ascii=False))
