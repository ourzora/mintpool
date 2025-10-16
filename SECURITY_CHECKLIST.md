# Checklist Sécurité (rapide)

- Pas de clés dans le code (utiliser .env + gitignore).
- Compiler avec optimizer activé, version fixée.
- Limiter les surfaces d'attaque (no owner superflu).
- Utiliser events pour les actions critiques.
- Tests unitaires sur les fonctions sensibles.
- Vérifier les overflows/underflows (>=0.8.x protège, mais rester prudent).
