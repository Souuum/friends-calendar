<script>
  import { page } from '$app/stores';
</script>

<aside class="side-menu bg-gray-100 border-r border-gray-300 p-4 min-h-screen">
  <nav>
    <section class="calendars">
      <h2 class="text-sm font-semibold mb-4 text-gray-800">Calendars</h2>
      <ul class="list-none p-0 m-0">
        <li class="mb-2">
          <a
            href="/calendars/my"
            class="block px-4 py-2 text-gray-600 no-underline rounded transition-colors duration-200"
            class:bg-blue-500={$page.url.pathname === '/calendars/my'}
            class:text-white={$page.url.pathname === '/calendars/my'}
            class:hover:bg-gray-300={$page.url.pathname !== '/calendars/my'}
          >
            My Calendar
          </a>
        </li>
        <li class="mb-2">
          <a
            href="/calendars/friends"
            class="block px-4 py-2 text-gray-600 no-underline rounded transition-colors duration-200"
            class:bg-blue-500={$page.url.pathname === '/calendars/friends'}
            class:text-white={$page.url.pathname === '/calendars/friends'}
            class:hover:bg-gray-300={$page.url.pathname !== '/calendars/friends'}
          >
            Friends' Calendars
          </a>
        </li>
      </ul>
    </section>
  </nav>
</aside>
