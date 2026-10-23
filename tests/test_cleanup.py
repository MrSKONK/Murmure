"""python -m unittest discover tests"""
import unittest

from src.cleanup import clean, fit_to_context

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


class RepeatsTest(unittest.TestCase):
    def test_stutter_removed(self):
        self.assertEqual(clean("Je pense que le le chat est là."), "Je pense que le chat est là.")
        self.assertEqual(clean("Il faut faut faut partir."), "Il faut partir.")

    def test_correct_repetitions_kept(self):
        for text in ("Nous nous sommes vus.", "Vous vous trompez.", "Si si, c'est vrai.", "Il est très très grand."):
            self.assertEqual(clean(text), text)

    def test_numbers_untouched(self):
        self.assertEqual(clean("Le score est 10 10."), "Le score est 10 10.")


class AddressesTest(unittest.TestCase):
    def test_email(self):
        self.assertEqual(clean("Écris à Nathan arobase gmail point com."), "Écris à nathan@gmail.com.")
        self.assertEqual(clean("jean point dupont tiret bas pro arobase orange point fr"),
                         "jean.dupont_pro@orange.fr")
        self.assertEqual(clean("marie tiret claire arobase exemple point org"), "marie-claire@exemple.org")

    def test_url(self):
        self.assertEqual(clean("Va sur www point google point fr pour voir."), "Va sur www.google.fr pour voir.")

    def test_ordinary_point_kept(self):
        for text in ("Le point com de cette affaire.", "Le deuxième point fr était bon.", "C'est un point important."):
            self.assertEqual(clean(text), text)

    def test_disabled(self):
        self.assertEqual(clean("nathan arobase gmail point com", addresses=False), "Nathan arobase gmail point com")


class ShortMessageTest(unittest.TestCase):
    def test_single_short_sentence_loses_period(self):
        self.assertEqual(clean("On se voit demain.", short_no_period=True), "On se voit demain")

    def test_kept_otherwise(self):
        self.assertEqual(clean("On se voit demain. Bonne soirée.", short_no_period=True),
                         "On se voit demain. Bonne soirée.")
        self.assertEqual(clean("Tu viens ?", short_no_period=True), "Tu viens ?")
        long_text = " ".join(f"mot{i}" for i in range(25)) + "."
        self.assertTrue(clean(long_text, short_no_period=True).endswith("."))
        self.assertEqual(clean("On se voit demain."), "On se voit demain.")


class FitToContextTest(unittest.TestCase):
    def test_unknown_context_unchanged(self):
        self.assertEqual(fit_to_context("Merci.", None), "Merci.")

    def test_start_of_field_or_line(self):
        self.assertEqual(fit_to_context("Merci.", ""), "Merci.")
        self.assertEqual(fit_to_context("Merci.", "Bonjour,\n"), "Merci.")

    def test_mid_sentence(self):
        self.assertEqual(fit_to_context("Je voulais te dire.", "Bonjour Marc, "), "je voulais te dire.")
        self.assertEqual(fit_to_context("Je voulais te dire.", "Bonjour Marc,"), " je voulais te dire.")

    def test_after_sentence(self):
        self.assertEqual(fit_to_context("Merci.", "C'est fait. "), "Merci.")
        self.assertEqual(fit_to_context("Merci.", "C'est fait."), " Merci.")

    def test_keeps_names_acronyms_addresses(self):
        self.assertEqual(fit_to_context("Kubernetes tourne.", "et ", ("Kubernetes",)), "Kubernetes tourne.")
        self.assertEqual(fit_to_context("API prête.", "mon "), "API prête.")
        self.assertEqual(fit_to_context("nathan@gmail.com", "écris à "), "nathan@gmail.com")

    def test_no_space_after_opening(self):
        self.assertEqual(fit_to_context("Bonjour", "dit («"), "bonjour")


if __name__ == "__main__":
    unittest.main()
