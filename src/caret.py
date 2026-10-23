"""Lit les quelques caractères situés juste avant le curseur dans l'application active (UI Automation, Windows).

Sert seulement à raccorder la dictée (majuscule, espace) ; rien n'est conservé ni envoyé.
Renvoie None quand l'application ne l'expose pas : la dictée est alors collée telle quelle."""
import threading

_UIA_TEXT_PATTERN = 10014
_START, _END = 0, 1          # TextPatternRangeEndpoint
_CHARACTER = 0               # TextUnit_Character


def _uia_module():
    import comtypes.client
    return comtypes.client.GetModule("UIAutomationCore.dll")


def _read(max_chars: int) -> str | None:
    import comtypes
    import comtypes.client

    comtypes.CoInitialize()
    try:
        uia_mod = _uia_module()
        uia = comtypes.client.CreateObject(uia_mod.CUIAutomation, interface=uia_mod.IUIAutomation)
        element = uia.GetFocusedElement()
        if not element or element.CurrentIsPassword:
            return None
        pattern = element.GetCurrentPattern(_UIA_TEXT_PATTERN)
        if not pattern:
            return None
        text_pattern = pattern.QueryInterface(uia_mod.IUIAutomationTextPattern)
        selection = text_pattern.GetSelection()
        if not selection or selection.Length < 1:
            return None
        caret = selection.GetElement(0)
        before = caret.Clone()
        before.MoveEndpointByRange(_END, caret, _START)
        before.MoveEndpointByUnit(_START, _CHARACTER, -max_chars)
        return before.GetText(max_chars)
    finally:
        comtypes.CoUninitialize()


def text_before_caret(max_chars: int = 12, timeout: float = 0.4) -> str | None:
    """Abandonne au-delà de `timeout` secondes : une application figée ne doit pas retarder la dictée."""
    result: list = [None]

    def work():
        try:
            result[0] = _read(max_chars)
        except Exception:
            result[0] = None

    t = threading.Thread(target=work, daemon=True)
    t.start()
    t.join(timeout)
    return None if t.is_alive() else result[0]


def self_test() -> str:
    """Pour l'autotest de l'exécutable : l'interface UI Automation est-elle utilisable ?"""
    import comtypes
    import comtypes.client

    comtypes.CoInitialize()
    try:
        uia_mod = _uia_module()
        comtypes.client.CreateObject(uia_mod.CUIAutomation, interface=uia_mod.IUIAutomation)
        return "ok"
    finally:
        comtypes.CoUninitialize()
