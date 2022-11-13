from typing import TYPE_CHECKING, Sequence

from trezor import log, wire, workflow
from trezor.enums import ButtonRequestType
from trezor.messages import ButtonAck, ButtonRequest

if TYPE_CHECKING:
    from typing import Any, Awaitable

    LayoutType = Awaitable[Any]
    PropertyType = tuple[str | None, str | bytes | None]
    ExceptionType = BaseException | type[BaseException]


async def button_request(
    ctx: wire.GenericContext,
    br_type: str,
    code: ButtonRequestType = ButtonRequestType.Other,
    pages: int | None = None,
) -> None:
    if __debug__:
        log.debug(__name__, "ButtonRequest.type=%s", br_type)
    workflow.close_others()
    if pages is not None:
        await ctx.call(ButtonRequest(code=code, pages=pages), ButtonAck)
    else:
        await ctx.call(ButtonRequest(code=code), ButtonAck)


async def interact(
    ctx: wire.GenericContext,
    layout: LayoutType,
    br_type: str,
    br_code: ButtonRequestType = ButtonRequestType.Other,
) -> Any:
    if layout.__class__.__name__ == "Paginated":
        from ..components.tt.scroll import Paginated

        assert isinstance(layout, Paginated)
        return await layout.interact(ctx, code=br_code)
    elif hasattr(layout, "in_unknown_flow") and layout.in_unknown_flow():  # type: ignore [Cannot access member "in_unknown_flow" for type "LayoutType"]
        # We cannot recognize before-hand how many pages the layout will have -
        # but we know for certain we want to paginate through them
        # TODO: could do something less hacky than sending 0 as page count
        # (create new ButtonRequest field)
        await button_request(ctx, br_type, br_code, pages=0)
        return await ctx.wait(layout)
    elif hasattr(layout, "page_count") and layout.page_count() > 1:  # type: ignore [Cannot access member "page_count" for type "LayoutType"]
        # We know for certain how many pages the layout will have
        await button_request(ctx, br_type, br_code, pages=layout.page_count())  # type: ignore [Cannot access member "page_count" for type "LayoutType"]
        return await ctx.wait(layout)
    else:
        await button_request(ctx, br_type, br_code)
        return await ctx.wait(layout)


def split_share_into_pages(share_words: Sequence[str], per_page: int = 4) -> list[str]:
    pages: list[str] = []
    current = ""

    for i, word in enumerate(share_words):
        if i % per_page == 0:
            if i != 0:
                pages.append(current)
            current = ""
        else:
            current += "\n"
        current += f"{i + 1}. {word}"

    if current:
        pages.append(current)

    return pages
