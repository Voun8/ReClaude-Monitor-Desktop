<script lang="ts">
  // 薄壳：HERO（当前账号）+ 切换账号区。两区各自直读 monitor store，无自身逻辑。
  import HeroCard from "./HeroCard.svelte";
  import AccountSwitcher from "./AccountSwitcher.svelte";
</script>

<HeroCard />
<AccountSwitcher />
