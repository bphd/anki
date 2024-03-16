// Copyright: Ankitects Pty Ltd and contributors
// License: GNU AGPL, version 3 or later; http://www.gnu.org/licenses/agpl.html
import { getDeckConfigsForUpdate } from "@generated/backend";
import { DeckOptionsState } from "../lib";
import type { PageLoad } from "./$types";

export const load = (async ({ fetch, url, params }) => {
    const deckId = Number(params.deckId);

    const did = BigInt(deckId);
    const info = await getDeckConfigsForUpdate({ did });
    const state = new DeckOptionsState(BigInt(did), info);

    return { state };
}) satisfies PageLoad;
