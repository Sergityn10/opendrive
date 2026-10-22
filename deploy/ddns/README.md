# Actualizador dinámico de DNS (DDNS)

Si tu proveedor de internet (ISP) te asigna una dirección IP pública dinámica (que puede cambiar al reiniciar el router o periódicamente) y el router de tu casa no tiene DDNS activo, este servicio actualiza automáticamente los registros DNS de tu dominio cada vez que tu IP cambie.

## Cómo usarlo

1. Copia `config.json.example` a `config.json` en este mismo directorio:
   ```powershell
   copy config.json.example config.json
   ```

2. Edita `config.json` y deja únicamente el proveedor que utilices con tus credenciales:
   - **Cloudflare**: Token de API con permisos de edición de zona DNS + Zone ID.
   - **DuckDNS**: Nombre de dominio y token.
   - **Namecheap**: Dominio y contraseña de Dynamic DNS.
   - **Porkbun**, **OVH**, **GoDaddy**, etc.: consulta la documentación de [ddns-updater](https://github.com/qdm12/ddns-updater).

3. Inicia OpenDrive con el perfil `ddns`:
   ```bash
   docker compose --profile ddns up -d
   ```

4. Para ver el estado de las actualizaciones:
   ```bash
   docker compose logs -f ddns-updater
   ```
