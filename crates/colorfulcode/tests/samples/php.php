<?php
declare(strict_types=1);

namespace App\Http;

use App\Models\User;

#[Route('/users/{id}')]
final class UserController extends Controller
{
    private const PER_PAGE = 20;

    public function __construct(private readonly UserRepository $users) {}

    public function show(int $id): ?User
    {
        $user = $this->users->find($id) ?? throw new NotFoundException("User {$id} not found");
        echo "Hello, $user->name\n";
        return match (true) {
            $user->isAdmin() => $user,
            default => null,
        };
    }
}

$sql = <<<SQL
    SELECT * FROM users WHERE id = :id
    SQL;
?>
<p>Total: <?= count($items) ?></p>
