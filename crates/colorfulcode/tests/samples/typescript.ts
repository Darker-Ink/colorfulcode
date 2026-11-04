// Types, generics and a little runtime code.
export interface User {
  id: number;
  name: string;
  email?: string;
  readonly roles: Array<"admin" | "user">;
}

type Handler<T> = (event: T) => Promise<void>;
enum Status { Active = "active", Disabled = "disabled" }

export abstract class Repository<T extends { id: number }> {
  #cache = new Map<number, T>();
  protected abstract load(id: number): Promise<T | undefined>;

  async get(id: number): Promise<T> {
    const cached = this.#cache.get(id);
    if (cached) return cached;
    const value = await this.load(id);
    if (!value) throw new Error(`Missing record ${id}`);
    this.#cache.set(id, value);
    return value;
  }
}

const isAdmin = (user: User): user is User & { roles: ["admin"] } => user.roles.includes("admin");
const generic = <T>(input: T) => input;
let count: number = 0x1f + 1e3 + 10n;
console.log(Status.Active, isAdmin, generic<string>("ok") satisfies string);
