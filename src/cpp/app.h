#pragma once

class ActorRegistry;

namespace shoppinglist {

void
connectAboutToQuit(ActorRegistry& registry);

void
registerQmlTypes();

} // namespace shoppinglist
