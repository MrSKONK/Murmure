"""Logique de choix des corrections Grammalecte, sur des erreurs relevées par tools/grammalecte_probe.py."""
import unittest

from src.grammar import _homophone_test, apply_corrections, sound_key

same = _homophone_test(object())   # clé phonétique seule (sans le module de Grammalecte)


def err(text, span, sugg, typ, rule="r", nth=0, msg=""):
    start = -1
    for _ in range(nth + 1):
        start = text.index(span, start + 1)
    return {"nStart": start, "nEnd": start + len(span), "aSuggestions": sugg, "sType": typ, "sRuleId": rule,
            "sMessage": msg}


class SoundKeyTest(unittest.TestCase):
    def test_homophones(self):
        for a, b in [("mange", "mangent"), ("rouge", "rouges"), ("enfant", "enfants"), ("garé", "garés"),
                     ("manger", "mangé"), ("Elle", "Elles"), ("Ils", "Il"), ("a", "as"), ("a", "à"), ("Ou", "Où"),
                     ("Ces", "C’est"), ("sa", "ça"), ("c'est", "s'est"), ("dise", "dises"), ("vert", "verre"),
                     ("les", "lé"), ("ses", "ces")]:
            self.assertEqual(sound_key(a), sound_key(b), (a, b))

    def test_different_sounds(self):
        for a, b in [("sont", "est"), ("de", "des"), ("le", "les"), ("elle", "est"), ("test", "tes")]:
            self.assertNotEqual(sound_key(a), sound_key(b), (a, b))


class ApplyTest(unittest.TestCase):
    def test_simple_homophone_fixes(self):
        t = "Les voitures rouge sont garé devant chez moi."
        errors = [err(t, "rouge", ["rouges"], "gn", "g3__gn_les_2m__b1_a4_1"),
                  err(t, "garé", ["garés"], "ppas", "gv1__ppas_être_accord_plur__b1_a1_1")]
        self.assertEqual(apply_corrections(t, errors, same), "Les voitures rouges sont garés devant chez moi.")

    def test_alternatives_only_one_applied(self):
        t = "Ils mange des pommes."
        errors = [err(t, "mange", ["mangent"], "conj", "g2__conj_ils__b1_a1_1"),
                  err(t, "Ils", ["Il"], "conj", "g2__conj_ils__b1_a2_1")]
        self.assertEqual(apply_corrections(t, errors, same), "Ils mangent des pommes.")

    def test_alternative_chosen_by_sound(self):
        t = "Elle se sont trompés de route."
        errors = [err(t, "sont", ["est"], "conj", "gv2__conj_elle__b1_a1_1"),
                  err(t, "Elle", ["Elles"], "conj", "gv2__conj_elle__b1_a2_1")]
        self.assertEqual(apply_corrections(t, errors, same), "Elles se sont trompés de route.")

    def test_non_homophone_refused(self):
        t = "Elle se sont vues."
        self.assertEqual(apply_corrections(t, [err(t, "sont", ["est"], "conj")], same), t)

    def test_several_suggestions_refused(self):
        t = "Je suis aller au marché."
        self.assertEqual(apply_corrections(t, [err(t, "aller", ["allé", "allée"], "ppas")], same), t)

    def test_unsafe_types_ignored(self):
        t = "C'est quoi ?"
        errors = [err(t, "C'", ["C’"], "apos", "apostrophe_typographique"),
                  err(t, " ?", [" ?"], "nbsp", "nbsp_avant_double_ponctuation")]
        self.assertEqual(apply_corrections(t, errors, same), t)

    def test_straight_apostrophe_kept(self):
        t = "Ces un bon moyen."
        self.assertEqual(apply_corrections(t, [err(t, "Ces", ["C’est"], "conf")], same), "C'est un bon moyen.")

    def test_without_phonetics_alternatives_skipped(self):
        t = "Ils mange des pommes et il va a Paris."
        errors = [err(t, "mange", ["mangent"], "conj", "g2__conj_ils__b1_a1_1"),
                  err(t, "Ils", ["Il"], "conj", "g2__conj_ils__b1_a2_1"),
                  err(t, "a Paris", ["à Paris"], "conf", "g3__conf_à_a__b3_a1_1")]
        self.assertEqual(apply_corrections(t, errors, None), "Ils mange des pommes et il va à Paris.")

    def test_contradiction_left_alone(self):
        # Erreurs réelles de Grammalecte 2.3.0 (tools/grammalecte_probe.py)
        t = "Les résultats que j'ai obtenu sont bon."
        errors = [
            err(t, "sont", ["son"], "conf", "g2__conf_son_sont__b4_a1_1",
                msg="Confusion probable : “sont” est la forme verbale de “être” à la 3ᵉ personne du pluriel."),
            err(t, "obtenu", ["obtenus"], "ppas", "gv1__ppas_det_plur_COD_que_avoir__b1_a2_1",
                msg="Si “Les résultats…” est bien le COD du participe passé “obtenu”, celui-ci devrait être au pluriel."),
            err(t, "bon", ["bons"], "ppas", "gv1__ppas_être_accord_plur__b1_a1_1",
                msg="Accord avec la forme verbale “sont” : “bon” devrait être au pluriel."),
        ]
        self.assertEqual(apply_corrections(t, errors, same), "Les résultats que j'ai obtenus sont bons.")

    def test_alternatives_citing_each_other_still_fixed(self):
        t = "Ils mange des pommes."
        errors = [err(t, "mange", ["mangent"], "conj", "g2__conj_ils__b1_a1_1", msg="Accord avec “ils”."),
                  err(t, "Ils", ["Il"], "conj", "g2__conj_ils__b1_a2_1", msg="Accord avec le terme “mange”.")]
        self.assertEqual(apply_corrections(t, errors, same), "Ils mangent des pommes.")

    def test_overlaps_and_bad_offsets_ignored(self):
        t = "abc def"
        errors = [{"nStart": 0, "nEnd": 3, "aSuggestions": ["x"], "sType": "gn", "sRuleId": "a"},
                  {"nStart": 2, "nEnd": 5, "aSuggestions": ["y"], "sType": "gn", "sRuleId": "b"},
                  {"nStart": 5, "nEnd": 99, "aSuggestions": ["z"], "sType": "gn", "sRuleId": "c"}]
        self.assertEqual(apply_corrections(t, errors, lambda a, b: True), "x def")


if __name__ == "__main__":
    unittest.main()
