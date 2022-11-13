# This file is part of the Trezor project.
#
# Copyright (C) 2012-2019 SatoshiLabs and contributors
#
# This library is free software: you can redistribute it and/or modify
# it under the terms of the GNU Lesser General Public License version 3
# as published by the Free Software Foundation.
#
# This library is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU Lesser General Public License for more details.
#
# You should have received a copy of the License along with this library.
# If not, see <https://www.gnu.org/licenses/lgpl-3.0.html>.

import json
from pathlib import Path
from typing import TYPE_CHECKING, Generator, List, Optional

import pytest

from trezorlib import btc, tools
from trezorlib.messages import ButtonRequestType

if TYPE_CHECKING:
    from trezorlib.debuglink import LayoutLines
    from trezorlib.debuglink import DebugLink, TrezorClientDebugLink as Client
    from trezorlib.messages import ButtonRequest
    from _pytest.mark.structures import MarkDecorator


# fmt: off
#                1      2     3    4      5      6      7     8      9    10    11    12
MNEMONIC12 = "alcohol woman abuse must during monitor noble actual mixed trade anger aisle"
MNEMONIC_SLIP39_BASIC_20_3of6 = [
    "extra extend academic bishop cricket bundle tofu goat apart victim enlarge program behavior permit course armed jerky faint language modern",
    "extra extend academic acne away best indicate impact square oasis prospect painting voting guest either argue username racism enemy eclipse",
    "extra extend academic arcade born dive legal hush gross briefing talent drug much home firefly toxic analysis idea umbrella slice",
]
MNEMONIC_SLIP39_BASIC_20_3of6_SECRET = "491b795b80fc21ccdf466c0fbc98c8fc"
# Shamir shares (128 bits, 2 groups from 1 of 1, 1 of 1, 3 of 5, 2 of 6)
MNEMONIC_SLIP39_ADVANCED_20 = [
    "eraser senior beard romp adorn nuclear spill corner cradle style ancient family general leader ambition exchange unusual garlic promise voice",
    "eraser senior ceramic snake clay various huge numb argue hesitate auction category timber browser greatest hanger petition script leaf pickup",
    "eraser senior ceramic shaft dynamic become junior wrist silver peasant force math alto coal amazing segment yelp velvet image paces",
    "eraser senior ceramic round column hawk trust auction smug shame alive greatest sheriff living perfect corner chest sled fumes adequate",
]
# Shamir shares (256 bits, 2 groups from 1 of 1, 1 of 1, 3 of 5, 2 of 6):
MNEMONIC_SLIP39_ADVANCED_33 = [
    "wildlife deal beard romp alcohol space mild usual clothes union nuclear testify course research heat listen task location thank hospital slice smell failure fawn helpful priest ambition average recover lecture process dough stadium",
    "wildlife deal acrobat romp anxiety axis starting require metric flexible geology game drove editor edge screw helpful have huge holy making pitch unknown carve holiday numb glasses survive already tenant adapt goat fangs",
]
# External entropy mocked as received from trezorlib.
EXTERNAL_ENTROPY = b"zlutoucky kun upel divoke ody" * 2
# fmt: on

TEST_ADDRESS_N = tools.parse_path("m/44h/1h/0h/0/0")
COMMON_FIXTURES_DIR = (
    Path(__file__).resolve().parent.parent / "common" / "tests" / "fixtures"
)


def parametrize_using_common_fixtures(*paths: str) -> "MarkDecorator":
    fixtures = []
    for path in paths:
        fixtures.append(json.loads((COMMON_FIXTURES_DIR / path).read_text()))

    tests = []
    for fixture in fixtures:
        for test in fixture["tests"]:
            test_id = test.get("name")
            if not test_id:
                test_id = test.get("description")
                if test_id is not None:
                    test_id = test_id.lower().replace(" ", "_")

            tests.append(
                pytest.param(
                    test["parameters"],
                    test["result"],
                    marks=pytest.mark.setup_client(
                        passphrase=fixture["setup"]["passphrase"],
                        mnemonic=fixture["setup"]["mnemonic"],
                    ),
                    id=test_id,
                )
            )

    return pytest.mark.parametrize("parameters, result", tests)


def generate_entropy(
    strength: int, internal_entropy: bytes, external_entropy: bytes
) -> bytes:
    """
    strength - length of produced seed. One of 128, 192, 256
    random - binary stream of random data from external HRNG
    """
    import hashlib

    if strength not in (128, 192, 256):
        raise ValueError("Invalid strength")

    if not internal_entropy:
        raise ValueError("Internal entropy is not provided")

    if len(internal_entropy) < 32:
        raise ValueError("Internal entropy too short")

    if not external_entropy:
        raise ValueError("External entropy is not provided")

    if len(external_entropy) < 32:
        raise ValueError("External entropy too short")

    entropy = hashlib.sha256(internal_entropy + external_entropy).digest()
    entropy_stripped = entropy[: strength // 8]

    if len(entropy_stripped) * 8 != strength:
        raise ValueError("Entropy length mismatch")

    return entropy_stripped


def recovery_enter_shares(
    debug: "DebugLink",
    shares: List[str],
    groups: bool = False,
    click_info: bool = False,
) -> Generator[None, "ButtonRequest", None]:
    """Perform the recovery flow for a set of Shamir shares.

    For use in an input flow function.
    Example:

    def input_flow():
        yield  # start recovery
        client.debug.press_yes()
        yield from recovery_enter_shares(client.debug, SOME_SHARES)
    """
    word_count = len(shares[0].split(" "))

    # Homescreen - proceed to word number selection
    yield
    debug.press_yes()
    # Input word number
    br = yield
    assert br.code == ButtonRequestType.MnemonicWordCount
    debug.input(str(word_count))
    # Homescreen - proceed to share entry
    yield
    debug.press_yes()
    # Enter shares
    for share in shares:
        br = yield
        assert br.code == ButtonRequestType.MnemonicInput
        # Enter mnemonic words
        for word in share.split(" "):
            debug.input(word)

        if groups:
            # Confirm share entered
            yield
            debug.press_yes()

        # Homescreen - continue
        # or Homescreen - confirm success
        yield

        if click_info:
            # Moving through the INFO button
            debug.press_info()
            yield
            debug.swipe_up()
            debug.press_yes()

        # Finishing with current share
        debug.press_yes()


def click_through(
    debug: "DebugLink", screens: int, code: ButtonRequestType = None
) -> Generator[None, "ButtonRequest", None]:
    """Click through N dialog screens.

    For use in an input flow function.
    Example:

    def input_flow():
        # 1. Confirm reset
        # 2. Backup your seed
        # 3. Confirm warning
        # 4. Shares info
        yield from click_through(client.debug, screens=4, code=ButtonRequestType.ResetDevice)
    """
    for _ in range(screens):
        received = yield
        if code is not None:
            assert received.code == code
        debug.press_yes()


def read_and_confirm_mnemonic(
    debug: "DebugLink", choose_wrong: bool = False
) -> Generator[None, "ButtonRequest", Optional[str]]:
    """Read a given number of mnemonic words from Trezor T screen and correctly
    answer confirmation questions. Return the full mnemonic.

    For use in an input flow function.
    Example:

    def input_flow():
        yield from click_through(client.debug, screens=3)

        mnemonic = yield from read_and_confirm_mnemonic(client.debug)
    """
    mnemonic: List[str] = []
    br = yield
    assert br.pages is not None
    for _ in range(br.pages - 1):
        mnemonic.extend(debug.read_reset_word().split())
        debug.swipe_up(wait=True)

    # last page is confirmation
    mnemonic.extend(debug.read_reset_word().split())
    debug.press_yes()

    # check share
    for _ in range(3):
        index = debug.read_reset_word_pos()
        if choose_wrong:
            debug.input(mnemonic[(index + 1) % len(mnemonic)])
            return None
        else:
            debug.input(mnemonic[index])

    return " ".join(mnemonic)


class ModelRLayout:
    """Layout shortcuts for Model R."""

    def __init__(self, layout: "LayoutLines") -> None:
        self.layout = layout

    def get_mnemonic_words(self) -> List[str]:
        """Extract mnemonic words from the layout lines.

        Example input: [..., '3. abuse', '4. must', '5. during', '6. monitor', '7. noble ', ...]
        Example output: ['abuse', 'must', 'during', 'monitor', 'noble']
        """
        words: List[str] = []
        for line in self.layout.lines:
            if "." in line:
                number, word = line.split(".", 1)
                if all(c.isdigit() for c in number):
                    words.append(word.strip())

        return words

    def get_word_index(self) -> int:
        """Extract currently asked mnemonic index.

        Example input: "Select word 3/12"
        Example output: 2
        """
        prompt = self.layout.lines[0]
        human_index = prompt.split(" ")[-1].split("/")[0]
        return int(human_index) - 1

    def get_current_word(self) -> str:
        """Extract currently selected word.

        Example input: "SELECT [Select(monitor)]"
        Example output: "monitor"
        """
        buttons = self.layout.lines[-1]
        return buttons.split("[Select(")[1].split(")]")[0]


def read_and_confirm_mnemonic_tr(
    debug: "DebugLink", choose_wrong: bool = False
) -> Generator[None, "ButtonRequest", Optional[str]]:
    mnemonic: List[str] = []
    br = yield
    assert br.pages is not None
    for _ in range(br.pages):
        layout = debug.wait_layout()

        words = ModelRLayout(layout).get_mnemonic_words()
        mnemonic.extend(words)
        debug.press_right()

    # check share
    for _ in range(3):
        yield
        layout = debug.wait_layout()
        index = ModelRLayout(layout).get_word_index()
        if choose_wrong:
            debug.input(mnemonic[(index + 1) % len(mnemonic)])
            return None
        else:
            correct_word = mnemonic[index]
            # Navigating to the correct word before confirming (for UI purposes)
            for _ in range(3):
                get_current_word = ModelRLayout(layout).get_current_word()
                if correct_word == get_current_word:
                    debug.input(correct_word)
                    break
                else:
                    debug.press_right()
                    layout = debug.wait_layout()
            else:
                raise RuntimeError("Correct word not found")

    return " ".join(mnemonic)


def get_test_address(client: "Client") -> str:
    """Fetch a testnet address on a fixed path. Useful to make a pin/passphrase
    protected call, or to identify the root secret (seed+passphrase)"""
    return btc.get_address(client, "Testnet", TEST_ADDRESS_N)


def get_text_from_paginated_screen(client: "Client", screen_count: int) -> str:
    """Aggregating screen text from more pages into one string."""
    text: str = client.debug.wait_layout().text
    for _ in range(screen_count - 1):
        client.debug.swipe_up()
        text += client.debug.wait_layout().text

    return text
