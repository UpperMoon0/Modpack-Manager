import os
import tempfile
import aiohttp
import aiofiles
import zipfile
import logging


logging.basicConfig(filename='app.log', filemode='w', format='%(name)s - %(levelname)s - %(message)s')


async def download(url, progress_callback=None):
    async with aiohttp.ClientSession() as session:
        async with session.get(url) as resp:
            file_name = os.path.basename(url)
            temp_dir = tempfile.gettempdir()
            file_path = os.path.join(temp_dir, file_name)

            size = int(resp.headers.get('content-length', 0))
            bytes_read = 0

            async with aiofiles.open(file_path, 'wb') as f:
                while True:
                    chunk = await resp.content.read(1024)
                    if not chunk:
                        break

                    await f.write(chunk)
                    bytes_read += len(chunk)

                    if progress_callback is not None:
                        progress_callback(bytes_read, size)

            return file_path


def install(path, downloaded_file_path, progress_callback=None):
    if not os.path.exists(path):
        raise Exception(f"Path {path} does not exist")

    instances_path = os.path.join(path, "instances")
    if not os.path.exists(instances_path):
        os.makedirs(instances_path)

    if not downloaded_file_path.endswith(".zip"):
        raise Exception(f"File {downloaded_file_path} is not a zip file")

    with zipfile.ZipFile(downloaded_file_path, 'r') as zip_ref:
        total_files = len(zip_ref.infolist())
        extracted_files = 0

        for file in zip_ref.infolist():
            logging.info(f'Extracting file: {file.filename}')
            zip_ref.extract(file, instances_path)
            extracted_files += 1

            if progress_callback is not None:
                progress_callback(extracted_files, total_files)

    os.remove(downloaded_file_path)
