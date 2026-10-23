"""python -m unittest discover tests"""
import unittest

from src.cleanup import clean

NBSP, NNBSP = " ", " "


class FillersTest(unittest.TestCase):
    def test_removed_with_their_comma(self):
        self.assertEqual(clean("Alors, euh, je pense."), "Alors, je pense.")

    def test_at_start_recapitalizes(self):
        self.assertEqual(clean("Euh, bonjour à tous."), "Bonjour à tous.")

    def test_before_final_point(self):
        self.assertEqual(clean("Je pense, euh."), "Je pense.")

    def test_disabled(self):
        self.assertEqual(clean("Alors, euh, je pense.", fillers=False), "Alors, euh, je pense.")

    def test_words_containing_filler_untouched(self):
        self.assertEqual(clean("Il est heureux d'être humble."), "Il est heureux d'être humble.")

    def test_only_fillers_gives_empty(self):
        self.assertEqual(clean("Euh… hum."), "")


class SpokenPunctuationTest(unittest.TestCase):
    def test_plain_commands(self):
        self.assertEqual(clean("Bonjour virgule comment ça va point d'interrogation"),
                         "Bonjour, comment ça va?")

    def test_absorbs_whisper_punctuation(self):
        self.assertEqual(clean("Bonjour, virgule, comment ça va ? Point d’interrogation."),
                         "Bonjour, comment ça va?")

    def test_newlines(self):
        self.assertEqual(clean("Merci point à la ligne cordialement"), "Merci.\nCordialement")
        self.assertEqual(clean("Merci. À la ligne. Cordialement."), "Merci.\nCordialement.")
        self.assertEqual(clean("Premier point nouveau paragraphe second point"),
                         "Premier point\n\nSecond point")

    def test_quotes_and_parentheses(self):
        self.assertEqual(
            clean("Il a dit, ouvrez les guillemets, bonjour, fermez les guillemets, puis il est parti."),
            "Il a dit, «bonjour», puis il est parti.")
        self.assertEqual(clean("Il attend ouvrez la parenthèse peut-être fermez la parenthèse demain."),
                         "Il attend (peut-être) demain.")

    def test_bare_point_kept(self):
        text = "Le prix est de 3,5 euros. Le point important est là."
        self.assertEqual(clean(text), text)

    def test_disabled(self):
        self.assertEqual(clean("Bonjour virgule toi", commands=False), "Bonjour virgule toi")


class FrenchTypographyTest(unittest.TestCase):
    def test_spaces_before_double_punctuation(self):
        self.assertEqual(clean("Voici la liste deux-points du pain point virgule du lait point final", french=True),
                         f"Voici la liste{NBSP}: du pain{NNBSP}; du lait.")
        self.assertEqual(clean("Vraiment?Oui!", french=True), f"Vraiment{NNBSP}?Oui{NNBSP}!")

    def test_ordinary_spaces_when_nbsp_off(self):
        self.assertEqual(clean("Comment ça va?", french=True, nbsp=False), "Comment ça va ?")

    def test_times_and_urls_untouched(self):
        self.assertEqual(clean("Rendez-vous à 14:30 sur http://exemple.fr", french=True),
                         "Rendez-vous à 14:30 sur http://exemple.fr")

    def test_guillemets(self):
        self.assertEqual(clean("Il a dit«bonjour»hier.", french=True), f"Il a dit «{NBSP}bonjour{NBSP}» hier.")

    def test_not_applied_outside_french(self):
        self.assertEqual(clean("How are you?"), "How are you?")


if __name__ == "__main__":
    unittest.main()
